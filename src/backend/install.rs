use color_eyre::eyre::bail;
use color_eyre::eyre::eyre;
use color_eyre::eyre::Context;
use color_eyre::eyre::ContextCompat;
use color_eyre::{Result, Section};
use ipc_channel::ipc::IpcError;
use ipc_channel::ipc::IpcOneShotServer;
use ipc_channel::ipc::IpcSender;
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::io::BufReader;
use std::process::Command;
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tee_readwrite::TeeReader;

use crate::consts;
use crate::consts::repart_dir;
use crate::util::sys::check_uefi;
use crate::{
    backend::postinstall::PostInstallModule,
    backend::repart_output::RepartOutput,
    consts::{LIVE_BASE, ROOTFS_BASE},
    pages::destination::DiskInit,
    stage, util,
};

use super::repart_output::CryptData;

pub static IPC_CHANNEL: OnceLock<Mutex<IpcSender<InstallationMessage>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstallationState {
    pub langlocale: Option<String>,
    pub destination_disk: Option<DiskInit>,
    pub installation_type: Option<InstallationType>,
    pub mounttags: Option<crate::backend::custom::MountTargets>,
    pub postinstall: Vec<crate::backend::postinstall::Module>,
    pub encrypt: bool,
    pub tpm: bool,
    pub encryption_key: Option<String>,
}

// TODO: remove this after have support for anything other than chromebook
impl Default for InstallationState {
    fn default() -> Self {
        Self {
            tpm: false,
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
            encrypt: false,
            encryption_key: Option::default(),
        }
    }
}

/// IPC installation message for non-interactive mode
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum InstallationMessage {
    Status(String),
}

impl InstallationState {
    // todo: move methods from installationstate to here!
    #[allow(clippy::unwrap_in_result)]
    pub fn install_using_subprocess(
        &self,
        sender: &relm4::Sender<InstallationMessage>,
    ) -> Result<()> {
        if cfg!(debug_assertions) {
            let installation_state_dump_path =
                std::env::temp_dir().join("readymade-installation-state.json");
            tracing::debug!(
                "Dumping installation state to {}",
                installation_state_dump_path.display()
            );
            std::fs::write(installation_state_dump_path, serde_json::to_string(self)?)?;
        }

        let mut command = Command::new("pkexec");
        command.arg(std::env::current_exe()?);
        command.arg("--non-interactive");

        let (server, channel_id) = IpcOneShotServer::new()?;
        command.arg(channel_id);

        // list envars
        let envars = std::env::vars().collect::<Vec<_>>();

        for (key, value) in envars {
            if key.starts_with("REPART_") || key.starts_with("READYMADE_") {
                command.arg(format!("{key}={value}"));
            }
        }

        command.arg("NO_COLOR=1");

        command.arg(format!(
            "READYMADE_LOG={}",
            std::env::var("READYMADE_LOG").as_deref().unwrap_or("info")
        ));

        let mut stdout_logs: Vec<u8> = Vec::new();
        let mut stderr_logs: Vec<u8> = Vec::new();

        let (stdout_reader, stdout_writer) = os_pipe::pipe()?;
        let (stderr_reader, stderr_writer) = os_pipe::pipe()?;

        let tee_stdout = TeeReader::new(stdout_reader, &mut stdout_logs, false);
        let tee_stderr = TeeReader::new(stderr_reader, &mut stderr_logs, false);

        command
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
                (reader.lines()).for_each(|line| println!("| {}", line.unwrap()));
            });
            s.spawn(|| {
                let reader = BufReader::new(tee_stderr);
                (reader.lines()).for_each(|line| eprintln!("| {}", line.unwrap()));
            });
            s.spawn(|| -> Result<()> {
                let (receiver, _) = server.accept()?;

                let mut msg;
                while {
                    msg = receiver.recv().map(|msg| sender.emit(msg));
                    msg.is_ok()
                } {}
                _ = msg.map_err(|e| match e {
                    IpcError::Disconnected => {}
                    e => tracing::error!("Failed to receive message from subprocess: {e:?}"),
                });

                Ok(())
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
                        String::from_utf8_lossy(&strip_ansi_escapes::strip(&stdout_logs))
                    )
                })
                .with_note(|| {
                    format!(
                        "Stderr:\n{}",
                        String::from_utf8_lossy(&strip_ansi_escapes::strip(&stderr_logs))
                    )
                })),
            Err(e) => Err(eyre!("Failed to execute readymade non-interactively").wrap_err(e)),
        }
    }

    /// Copies the current config into a temporary directory, allowing them to be modified without
    /// affecting the original templates :D
    fn layer_configdir(cfg_dir: &Path) -> Result<PathBuf> {
        // /run/readymade-install
        let new_path = PathBuf::from("/run").join("readymade-install");
        std::fs::create_dir_all(&new_path)?;
        // Copy the contents of the cfg_dir to the new path
        util::fs::copy_dir(cfg_dir, "/run/readymade-install")?;

        Ok(new_path)
    }

    #[allow(clippy::unwrap_in_result)]
    #[tracing::instrument]
    pub fn install(&self) -> Result<()> {
        let inst_type = (self.installation_type.as_ref())
            .expect("A valid installation type should be set before calling install()");

        if let InstallationType::Custom = inst_type {
            let mut mounttags = self.mounttags.clone().unwrap();
            return crate::backend::custom::install_custom(self, &mut mounttags);
        }
        let blockdev = &(self.destination_disk.as_ref())
            .expect("A valid destination device should be set before calling install()")
            .devpath;

        tracing::info!("Layering repart templates");
        let cfgdir = Self::layer_configdir(&inst_type.cfgdir())?;

        // Let's write the encryption key to the keyfile
        let keyfile = std::path::Path::new(consts::LUKS_KEYFILE_PATH);
        if let Some(key) = &self.encryption_key {
            std::fs::write(keyfile, key)?;
        }

        // TODO: encryption
        self.enable_encryption(&cfgdir)?;
        let repart_out = stage!(mkpart {
            // todo: not freeze on error, show error message as err handler?
            Self::systemd_repart(blockdev, &cfgdir, self.encrypt && self.encryption_key.is_some())?
        });

        let repartcfg_export = super::export::SystemdRepartData::get_configs(&cfgdir)?;

        tracing::info!("Copying files done, Setting up system...");
        self.setup_system(
            repart_out,
            self.encryption_key.as_deref(),
            Some(repartcfg_export),
        )?;

        if let InstallationType::ChromebookInstall = inst_type {
            // FIXME: don't dd?
            Self::flash_submarine(blockdev)?;
            InstallationType::set_cgpt_flags(blockdev)?;
        }

        tracing::info!("Cleaning up state...");

        if let Some(_key) = &self.encryption_key {
            std::fs::remove_file(keyfile) // don't care if it fails
                .unwrap_or_else(|e| tracing::warn!("Failed to remove keyfile: {e}"));

            // Close all mapped LUKS devices if exists

            if let Ok(mut cache) = super::repart_output::MAPPER_CACHE.try_write() {
                if let Some(cache) = std::sync::Arc::get_mut(&mut cache) {
                    cache.clear();
                }
            }
        }

        tracing::info!("install() finished");
        Ok(())
    }

    #[tracing::instrument]
    fn setup_system(
        &self,
        output: RepartOutput,
        passphrase: Option<&str>,
        repart_cfgs: Option<super::export::SystemdRepartData>,
    ) -> Result<()> {
        // XXX: This is a bit hacky, but this function should be called before output.generate_fstab() for
        // the fstab generator to be correct, IF we're using encryption
        //
        // todo: Unfuck this
        let mut container = output.to_container(passphrase)?;

        let fstab = output.generate_fstab()?;

        // todo: Also handle custom installs? Needs more information
        let esp_node = check_uefi().then(|| output.get_esp_partition()).flatten();
        let xbootldr_node = output
            .get_xbootldr_partition()
            .context("No xbootldr partition found")?;

        let crypt_data = output.generate_cryptdata()?;

        let rdm_result = super::export::ReadymadeResult::new(self.clone(), repart_cfgs);

        container.run(|| {
            self._inner_sys_setup(fstab, crypt_data, esp_node, &xbootldr_node, rdm_result)
        })??;

        Ok(())
    }

    #[allow(clippy::unwrap_in_result)]
    #[tracing::instrument]
    pub fn _inner_sys_setup(
        &self,
        fstab: String,
        crypt_data: Option<CryptData>,
        esp_node: Option<String>,
        xbootldr_node: &str,
        state_dump: super::export::ReadymadeResult,
    ) -> Result<()> {
        // We will run the specified postinstall modules now
        let context = crate::backend::postinstall::Context {
            destination_disk: self.destination_disk.as_ref().unwrap().devpath.clone(),
            uefi: util::sys::check_uefi(),
            esp_partition: esp_node,
            xbootldr_partition: xbootldr_node.to_owned(),
            lang: self.langlocale.clone().unwrap_or_else(|| "C.UTF-8".into()),
            crypt_data: crypt_data.clone(),
        };

        tracing::info!("Writing /etc/fstab...");
        std::fs::write("/etc/fstab", fstab).wrap_err("cannot write to /etc/fstab")?;

        // Write the state dump to the chroot
        let state_dump_path = Path::new(crate::consts::READYMADE_STATE_PATH);
        let parent = state_dump_path
            .parent()
            .ok_or_else(|| eyre!("Invalid state dump path - no parent directory"))?;
        std::fs::create_dir_all(parent)
            .wrap_err("Failed to create parent directories for state dump")?;
        std::fs::write(
            state_dump_path,
            state_dump
                .export_string()
                .wrap_err("Failed to serialize state dump")?,
        )
        .wrap_err("Failed to write state dump file")?;

        if let Some(data) = crypt_data {
            tracing::info!("Writing /etc/crypttab...");
            std::fs::write("/etc/crypttab", data.crypttab)
                .wrap_err("cannot write to /etc/crypttab")?;
        }

        for module in &self.postinstall {
            tracing::debug!(?module, "Running module");
            module.run(&context)?;
        }

        Ok(())
    }

    #[tracing::instrument]
    fn flash_submarine(blockdev: &Path) -> Result<()> {
        tracing::debug!("Flashing submarine…");

        // Find target submarine partition
        let target_partition = lsblk::BlockDevice::list()?
            .into_iter()
            .find(|d| {
                d.is_part()
                    && d.disk_name().ok().as_deref()
                        == blockdev
                            .strip_prefix("/dev/")
                            .unwrap_or(&PathBuf::from(""))
                            .to_str()
                    && d.name.ends_with('2')
            })
            .ok_or_else(|| eyre!("Failed to find submarine partition"))?;

        let source_path = Path::new("/usr/share/submarine/submarine.kpart");
        let target_path = Path::new("/dev").join(&target_partition.name);

        let mut source_file = std::fs::File::open(source_path)?;
        let mut target_file = std::fs::OpenOptions::new().write(true).open(target_path)?;

        std::io::copy(&mut source_file, &mut target_file)?;
        target_file.sync_all()?;

        Ok(())
    }
    // As of February 14, 2025, I have disabled the `dd` method for flashing the submarine partition,
    // because we shouldn't really be dropping to shell commands for this kind of thing.
    //
    // The `dd` method is still here for reference if we ever need to use it again.
    //
    // See above for the new method of flashing the submarine partition,
    // programmatically copying the submarine partition to the target disk.
    //
    // - Cappy
    /*
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
    */

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

    fn set_encrypt_to_file(f: &str, tpm: bool) -> String {
        let mut f = serde_systemd_unit::parse(f).expect("cannot parse templates");
        let mut v = "key-file".to_owned();
        if tpm {
            v += "+tpm2";
        }
        (f.sections.get_mut("Partition").unwrap())
            .insert("Encrypt".to_owned(), serde_systemd_unit::Value::String(v));
        f.to_string()
    }

    #[allow(clippy::unwrap_in_result)]
    #[tracing::instrument]
    /// Enable encryption on the root partition config
    ///
    /// This method will modify the root partition config file to enable encryption
    ///
    /// Please use [`Self::layer_configdir`] before calling this method to avoid modifying the original config files
    fn enable_encryption(&self, cfgdir: &Path) -> Result<()> {
        if !self.encrypt {
            return Ok(());
        }
        let root_file = cfgdir.join("50-root.conf");
        let f = std::fs::read_to_string(&root_file)?;
        let f = Self::set_encrypt_to_file(&f, self.tpm);
        // We're gonna write directly to the file.
        //
        // Warning: Please don't use this method unless you're using layer_configdir
        std::fs::write(&root_file, f)?;

        // TODO: somehow actually use this config file
        Ok(())
    }

    #[allow(clippy::unwrap_in_result)]
    #[tracing::instrument]
    fn systemd_repart(
        blockdev: &Path,
        cfgdir: &Path,
        use_keyfile: bool,
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

        if use_keyfile {
            let keyfile_path = consts::LUKS_KEYFILE_PATH;
            tracing::debug!("Using keyfile for systemd-repart: {keyfile_path}");
            args.push("--key-file");
            args.push(keyfile_path);
        }

        args.extend(&[blockdev.to_str().unwrap()]);

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

        // Dump systemd-repart output to a file if in debug mode
        if cfg!(debug_assertions) {
            let repart_out_path = std::env::temp_dir().join("readymade-repart-output.json");
            tracing::debug!(
                "Dumping systemd-repart output to {}",
                repart_out_path.display()
            );
            std::fs::write(repart_out_path, out)?;
        }

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
    }

    fn set_cgpt_flags(blockdev: &Path) -> Result<()> {
        tracing::debug!("Setting cgpt flags");
        let blockdev_str = blockdev
            .to_str()
            .ok_or_else(|| eyre!("Invalid block device path"))?;
        let args = [
            ["add", "-i", "2", "-t"],
            ["kernel", "-P", "15", "-T"],
            ["1", "-S", "1", blockdev_str],
        ];
        let status = Command::new("cgpt").args(args.concat()).status()?;

        if !status.success() {
            bail!("cgpt command failed with exit code {:?}", status.code());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_set_encrypt_to_file() {
        let enc = super::InstallationState::set_encrypt_to_file("[Partition]\nType=root", false);
        assert!(enc.contains("Encrypt=key-file"),);
    }
}
