use color_eyre::eyre::bail;
use color_eyre::eyre::eyre;
use color_eyre::{Result, Section};
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::io::BufReader;
use std::process::Command;
use std::process::Stdio;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tee_readwrite::TeeReader;

use crate::consts::repart_dir;
use crate::{
    backend::postinstall::PostInstallModule,
    backend::repart_output::RepartOutput,
    consts::{LIVE_BASE, ROOTFS_BASE},
    pages::destination::DiskInit,
    stage, util,
};

#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallationState {
    pub langlocale: Option<String>,
    pub destination_disk: Option<DiskInit>,
    pub installation_type: Option<InstallationType>,
    pub mounttags: Option<crate::backend::custom::MountTargets>,
    pub postinstall: Vec<crate::backend::postinstall::Module>,
}

// TODO: remove this after have support for anything other than chromebook
impl Default for InstallationState {
    fn default() -> Self {
        Self {
            langlocale: Option::default(),
            destination_disk: Option::default(),
            installation_type: if let [one] = &crate::CONFIG.read().install.allowed_installtypes[..]
            {
                Some(*one)
            } else {
                None
            },
            mounttags: Option::default(),
            postinstall: crate::CONFIG.read().postinstall.clone(),
        }
    }
}

impl InstallationState {
    // todo: move methods from installationstate to here!
    #[allow(clippy::unwrap_in_result)]
    pub fn install_using_subprocess(&self) -> Result<()> {
        let mut command = Command::new("pkexec");
        command.arg(std::env::current_exe()?);

        if let Ok(value) = std::env::var("REPART_COPY_SOURCE") {
            command.arg(format!("REPART_COPY_SOURCE={value}"));
        }
        
        if let Ok(value) = std::env::var("READYMADE_REPART_DIR") {
            command.arg(format!("READYMADE_REPART_DIR={value}"));
        }

        if let Ok(value) = std::env::var("READYMADE_DRY_RUN") {
            command.arg(format!("READYMADE_DRY_RUN={value}"));
        }

        command.arg(format!(
            "READYMADE_LOG={}",
            std::env::var("READYMADE_LOG").as_deref().unwrap_or("trace")
        ));

        let mut stdout_logs: Vec<u8> = Vec::new();
        let mut stderr_logs: Vec<u8> = Vec::new();

        let (stdout_reader, stdout_writer) = os_pipe::pipe()?;
        let (stderr_reader, stderr_writer) = os_pipe::pipe()?;

        let tee_stdout = TeeReader::new(stdout_reader, &mut stdout_logs, false);
        let tee_stderr = TeeReader::new(stderr_reader, &mut stderr_logs, false);

        command
            // .arg(std::env::current_exe()?)
            .arg("--non-interactive")
            .stdin(std::process::Stdio::piped())
            .stdout(stdout_writer)
            .stderr(stderr_writer);

        let mut res = command.spawn()?;

        {
            let mut child_stdin = res.stdin.take().expect("can't take stdin");
            child_stdin.write_all(serde_json::to_string(self)?.as_bytes())?;
        };

        print!("┌─ BEGIN: Readymade subprocess logs\n│ ");
        let res = std::thread::scope(|s| {
            s.spawn(|| {
                let reader = BufReader::new(tee_stdout);
                reader
                    .lines()
                    .for_each(|line| println!("| {}", line.unwrap()));
            });
            s.spawn(|| {
                let reader = BufReader::new(tee_stderr);
                reader
                    .lines()
                    .for_each(|line| eprintln!("| {}", line.unwrap()));
            });

            let result = res.wait_with_output();
            drop(command);
            result
        });
        println!("└─ END OF Readymade subprocess logs");

        match res {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(eyre!("Readymade subprocess failed")
                .with_note(|| output.status.to_string())
                .with_note(|| {
                    format!(
                        "Stdout:\n{}",
                        strip_ansi_escapes::strip_str(
                            String::from_utf8(stdout_logs).expect("stdout is not valid UTF-8")
                        )
                    )
                })
                .with_note(|| {
                    format!(
                        "Stderr:\n{}",
                        strip_ansi_escapes::strip_str(
                            String::from_utf8(stderr_logs).expect("stderr is not valid UTF-8")
                        )
                    )
                })),
            Err(e) => Err(eyre!("Failed to execute readymade non-interactively").wrap_err(e)),
        }
    }

    #[allow(clippy::unwrap_in_result)]
    #[tracing::instrument]
    pub fn install(&self) -> Result<()> {
        let inst_type = self
            .installation_type
            .as_ref()
            .expect("A valid installation type should be set before calling install()");

        if let InstallationType::Custom = inst_type {
            let mut mounttags = self.mounttags.clone().unwrap();
            return crate::backend::custom::install_custom(self, &mut mounttags);
        }
        let blockdev = &self
            .destination_disk
            .as_ref()
            .expect("A valid destination device should be set before calling install()")
            .devpath;
        let cfgdir = inst_type.cfgdir();
        let repart_out = stage!("Creating partitions and copying files" {
            // todo: not freeze on error, show error message as err handler?
            Self::systemd_repart(blockdev, &cfgdir)?
        });

        tracing::info!("Copying files done, Setting up system...");
        self.setup_system(repart_out)?;

        if let InstallationType::ChromebookInstall = inst_type {
            // FIXME: don't dd?
            Self::dd_submarine(blockdev)?;
            InstallationType::set_cgpt_flags(blockdev)?;
        }
        tracing::info!("install() finished");
        Ok(())
    }

    #[tracing::instrument]
    fn setup_system(&self, output: RepartOutput) -> Result<()> {
        let mut container = output.to_container()?;

        container.run(|| self._inner_sys_setup())?
    }

    #[tracing::instrument]
    pub fn _inner_sys_setup(&self) -> Result<()> {
        // We will run the specified postinstall modules now
        let context = crate::backend::postinstall::Context {
            destination_disk: self.destination_disk.as_ref().unwrap().devpath.clone(),
            uefi: util::sys::check_uefi(),
        };

        for module in &self.postinstall {
            module.run(&context)?;
        }

        Ok(())
    }

    #[tracing::instrument]
    fn dd_submarine(blockdev: &Path) -> Result<()> {
        tracing::debug!("dd-ing submarine…");
        if !Command::new("dd")
            .arg("if=/usr/share/submarine/submarine.kpart")
            .arg(format!(
                "of=/dev/{}",
                lsblk::BlockDevice::list()?
                    .into_iter()
                    .find(|d| d.is_part()
                        && d.disk_name().ok().as_deref()
                            == blockdev
                                .strip_prefix("/dev/")
                                .unwrap_or(&PathBuf::from(""))
                                .to_str()
                        && d.name.ends_with('2'))
                    .ok_or_else(|| eyre!("Failed to find submarine partition"))?
                    .name
            ))
            .arg("status=progress")
            .status()?
            .success()
        {
            return Err(eyre!("Failed to dd submarine, non-zero exit code"));
        }
        Ok(())
    }

    /// Mount a device or file to /mnt/live-base
    fn mount_dev(dev: &str) -> std::io::Result<sys_mount::Mount> {
        const MOUNTPOINT: &str = "/mnt/live-base";
        std::fs::create_dir_all(MOUNTPOINT)?;
        sys_mount::Mount::builder().mount(dev, MOUNTPOINT)
    }

    pub fn determine_copy_source() -> String {
        const FALLBACK: &str = "/mnt/live-base";
        // We'll be using a new feature from systemd 255 (relative repart copy source)
        // to copy the repartitioning definitions from the live base to the target disk

        // environment variable override. This is documented in HACKING.md

        std::env::var("REPART_COPY_SOURCE").map_or_else(|_| if std::fs::metadata(ROOTFS_BASE)
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            tracing::info!(
                "Using {} as copy source, as it exists presumably due to raw rootfs in dracut",
                ROOTFS_BASE
            );
            ROOTFS_BASE.to_owned()
        }
        // if we can mount /dev/mapper/live-base, we'll use that as the copy source
        else {
            match Self::mount_dev(LIVE_BASE) {
                Ok(mount) => {
                    let m = mount.target_path().to_string_lossy().to_string();
                    tracing::info!("Mounted live-base at {m}");
                    m
                }
                Err(e) => {
                    tracing::error!("Failed to mount `{LIVE_BASE}`, using `{FALLBACK}` as copy source anyway... ({e})");
                    FALLBACK.to_owned()
                }
            }
        }, |copy_source| {
            tracing::info!("Using REPART_COPY_SOURCE override: {copy_source}");
            let copy_source = Path::new(&copy_source.trim()).canonicalize().unwrap();

            if copy_source == Path::new("/") {
                tracing::warn!("REPART_COPY_SOURCE is set to `/`, this is likely a mistake. Copying entire host root filesystem to target disk...");
            }

            // convert back to string, may cause performance issues but it's not a big deal
            copy_source.to_string_lossy().to_string()
        })
    }

    // todo: Generate custom repart partitioning definitions in case the user wants to use a custom partitioning scheme
    #[allow(clippy::unwrap_in_result)]
    #[tracing::instrument]
    fn systemd_repart(
        blockdev: &Path,
        cfgdir: &Path,
    ) -> Result<crate::backend::repart_output::RepartOutput> {
        let copy_source = Self::determine_copy_source();

        let dry_run =
            std::env::var("READYMADE_DRY_RUN").map_or(cfg!(debug_assertions), |v| v == "1");
        let dry_run = if dry_run { "yes" } else { "no" };

        let mut args = vec![
            "--dry-run",
            dry_run,
            "--definitions",
            cfgdir.to_str().unwrap(),
            "--empty",
            "force",
            "--offline",
            "false",
            "--copy-source",
            &copy_source,
            "--json",
            "pretty",
        ];

        // if systemd_version()? >= 256 {
        //     args.push("--generate-fstab");
        //     args.push("/dev/stdout");
        // }

        args.push(blockdev.to_str().unwrap());

        tracing::debug!(?dry_run, ?args, "Running systemd-repart");

        let repart_cmd = Command::new("systemd-repart")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| eyre!("systemd-repart failed").wrap_err(e))?;

        if !repart_cmd.status.success() {
            bail!(
                "systemd-repart errored with status code {:?}",
                repart_cmd.status.code()
            );
        }

        let out = std::str::from_utf8(&repart_cmd.stdout)?;

        // todo: wait for systemd 256 or genfstab magic
        tracing::debug!(out, "systemd-repart finished");
        Ok(serde_json::from_str(out)?)
    }
}

impl InstallationType {
    fn cfgdir(&self) -> PathBuf {
        match self {
            Self::ChromebookInstall => repart_dir().join("chromebook"),
            Self::WholeDisk => repart_dir().join("wholedisk"),
            Self::DualBoot(_) => todo!(),
            Self::Custom => unreachable!(),
        }
        .into()
    }

    fn set_cgpt_flags(blockdev: &Path) -> Result<()> {
        tracing::debug!("Setting cgpt flags");
        let blockdev_str = blockdev
            .to_str()
            .ok_or_else(|| eyre!("Invalid block device path"))?;
        let status = Command::new("cgpt")
            .args([
                "add",
                "-i",
                "2",
                "-t",
                "kernel",
                "-P",
                "15",
                "-T",
                "1",
                "-S",
                "1",
                blockdev_str,
            ])
            .status()?;

        if !status.success() {
            bail!("cgpt command failed with exit code {:?}", status.code());
        }
        Ok(())
    }
}
