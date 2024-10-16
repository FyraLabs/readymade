use color_eyre::eyre::bail;
use color_eyre::eyre::eyre;
use color_eyre::{Result, Section};
use itertools::Itertools;
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

use crate::util::{exist_then, exist_then_read_dir};
use crate::{
    backend::repart_output::RepartOutput,
    pages::destination::DiskInit,
    stage,
    util::{self, LIVE_BASE},
};

const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";

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
        }
    }
}

impl InstallationState {
    // todo: move methods from installationstate to here!
    #[allow(clippy::unwrap_in_result)]
    pub fn install_using_subprocess(&self) -> Result<()> {
        let mut command = Command::new("pkexec");
        command.arg("env");

        if let Ok(value) = std::env::var("REPART_COPY_SOURCE") {
            command.arg(format!("REPART_COPY_SOURCE={value}"));
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
            .arg(std::env::current_exe()?)
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
        setup_system(repart_out)?;

        if let InstallationType::ChromebookInstall = inst_type {
            InstallationType::set_cgpt_flags(blockdev)?;
        }
        tracing::info!("install() finished");
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

        std::env::var("REPART_COPY_SOURCE").map_or_else(|_| if std::fs::metadata(crate::util::ROOTFS_BASE)
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            tracing::info!(
                "Using {} as copy source, as it exists presumably due to raw rootfs in dracut",
                crate::util::ROOTFS_BASE
            );
            crate::util::ROOTFS_BASE.to_owned()
        }
        // if we can mount /dev/mapper/live-base, we'll use that as the copy source
        else {
            match Self::mount_dev(crate::util::LIVE_BASE) {
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

#[tracing::instrument]
pub fn setup_system(output: RepartOutput) -> Result<()> {
    let mut container = output.to_container()?;

    // The reason we're checking for UEFI here is because we want to check the current
    // system's boot mode before we install GRUB, not check inside the container
    let uefi = util::check_uefi();
    container.run(|| _inner_sys_setup(uefi))?
}

#[tracing::instrument]
pub fn _inner_sys_setup(uefi: bool) -> Result<()> {
    if uefi {
        // The reason why we don't do grub2-install here is because for
        // Fedora specifically, the install script simply plops in
        // a pre-built GRUB binary in the ESP that looks for the stage 1
        // config in /boot/efi/EFI/fedora/grub.cfg
        // The following config then redirects to the actual stage 2 config located
        // in /boot/grub2/grub.cfg
        // This is actually done to support BLS entries properly on their end

        // todo: Add support for systemd-boot
        std::fs::create_dir_all("/boot/efi/EFI/fedora")?;

        stage!("Generating stage 1 grub.cfg in ESP..." {
            let mut grub_cfg = std::fs::File::create("/boot/efi/EFI/fedora/grub.cfg")?;
            // let's search for an xbootldr label
            // because we never know what the device will be
            // see the compile time included config file
            grub_cfg.write_all(include_bytes!("../templates/fedora-grub.cfg"))?;
        });

        stage!("Generating stage 2 grub.cfg in /boot/grub2/grub.cfg..." {
            let grub_cmd_status = Command::new("grub2-mkconfig")
                .arg("-o")
                .arg("/boot/grub2/grub.cfg")
                .status()?;

            if !grub_cmd_status.success() {
                bail!("grub2-mkconfig failed with exit code {:?}", grub_cmd_status.code());
            }
        });
    } else {
        stage!("Installing BIOS Grub2" {
            util::grub2_install_bios(crate::INSTALLATION_STATE.read().destination_disk.as_ref().unwrap().devpath.as_path())?;
        });
    }

    stage!("Cleaning up /boot partition..." {
        for file in std::fs::read_dir("/boot")?.flatten().map(|entry| entry.path()) {
            let file_name = file.file_name().unwrap().to_str().unwrap();
            if file_name.starts_with("initramfs") || file_name.starts_with("vmlinuz") {
                tracing::debug!(?file, "Removing kernel file");
                std::fs::remove_file(file)?;
            }
        }

        for file in std::fs::read_dir("/boot/loader/entries")?.flatten().map(|entry| entry.path()) {
            tracing::debug!(?file, "Removing BLS entry");
            std::fs::remove_file(file)?;
        }
    });

    // Reinstall kernel
    //
    // Here we're going to reinstall the kernel with an initramfs optimized
    // for the new system configuration. We'll be doing this by using kernel-install
    //
    // which runs all the necessary hooks to generate the initramfs and install the kernel properly.
    //
    // As a bonus, it also generates the BLS entries for us.
    stage!("Reinstalling kernel" {
        // list all kernels in /lib/modules
        // suggestion: Switch to using kernel-install --json=short for parsing
        let kernel_vers = std::fs::read_dir("/lib/modules")?
            .map(|entry| entry.unwrap().file_name())
            .collect_vec();

        tracing::info!(?kernel_vers, "Kernel versions found");

        // We're gonna just install the first kernel we find, so let's do that
        let kver = kernel_vers.first().unwrap().to_str().unwrap();

        // install kernel

        let kernel_install_cmd_status = Command::new("kernel-install")
            .arg("add")
            .arg(kver)
            .arg(format!("/lib/modules/{kver}/vmlinuz"))
            .arg("--verbose")
            .status()?;

        if !kernel_install_cmd_status.success() {
            bail!("kernel-install failed with exit code {:?}", kernel_install_cmd_status.code());
        }
    });
    // if systemd_version()? <= 256 {
    //     stage!("Generating /etc/fstab..." {
    //         let mut fstab = std::fs::File::create("/etc/fstab")?;
    //         fstab.write_all(output.generate_fstab().as_bytes())?;
    //     });
    // }

    stage!("Regenerating initramfs" {
        // We assume the installation wouldn't be used on another system (false only if you install
        // on something like a USB stick anyway)
        // → reduce size of initramfs aggressively for faster boot times
        //
        // on my system this reduces the size from 170M down to 43M.
        // — mado
        let dracut_cmd_status = Command::new("dracut").args([
            "--force",
            "--parallel",
            "--regenerate-all",
            "--hostonly",
            "--strip",
            "--aggressive-strip",
        ]).status()?;

        if !dracut_cmd_status.success() {
            bail!("dracut failed with exit code {:?}", dracut_cmd_status.code());
        }
    });

    stage!("Initializing system" {
        _initialize_system()?;
    });

    stage!("Setting SELinux contexts..." {
        let setfiles_cmd_status = Command::new("setfiles")
            .args(["-e", "/proc", "-e", "/sys"])
            .arg("/etc/selinux/targeted/contexts/files/file_contexts")
            .arg("/")
            .status()?;

        if !setfiles_cmd_status.success() {
            bail!("dracut failed with exit code {:?}", setfiles_cmd_status.code());
        }
    });

    Ok(())
}

/// Initialize the system after installation
/// This function is moved to a separate function to allow for cleaner code
#[tracing::instrument]
fn _initialize_system() -> color_eyre::Result<()> {
    exist_then(std::fs::remove_file("/var/lib/systemd/random-seed"))?;
    // We're gonna make an empty machine-id file so that systemd can generate a new one
    std::fs::File::create("/etc/machine-id")?;

    // wipe NetworkManager state
    exist_then(std::fs::remove_dir_all(
        "/etc/NetworkManager/system-connections",
    ))?;
    std::fs::create_dir_all("/etc/NetworkManager/system-connections")?;

    // todo: Copy over NetworkManager state from current livesys

    // wipe temporary RPM database
    exist_then_read_dir("/var/lib/rpm")?
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("__db"))
        .map(|entry| entry.path())
        .try_for_each(std::fs::remove_file)?;

    // wipe temporary dnf cache
    exist_then(std::fs::remove_dir_all("/var/cache/dnf"))?;

    // todo: set locale and timezone from config

    Ok(())
}

impl InstallationType {
    fn cfgdir(&self) -> PathBuf {
        match self {
            Self::ChromebookInstall => const_format::concatcp!(REPART_DIR, "chromebookinstall"),
            Self::WholeDisk => const_format::concatcp!(REPART_DIR, "wholedisk"),
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
                "1",
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
