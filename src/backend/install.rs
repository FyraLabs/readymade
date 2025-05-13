use ipc_channel::ipc::{IpcError, IpcOneShotServer, IpcSender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

use crate::{
    backend::{
        postinstall::PostInstallModule,
        repart_output::{CryptData, RepartOutput},
    },
    consts::{self, repart_dir, LIVE_BASE, ROOTFS_BASE},
    pages::destination::DiskInit,
    prelude::*,
    stage,
    util::{self, sys::check_uefi},
};

pub static IPC_CHANNEL: OnceLock<Mutex<IpcSender<InstallationMessage>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct InstallationState {
    pub langlocale: Option<String>,
    pub destination_disk: Option<DiskInit>,
    pub installation_type: Option<InstallationType>,
    pub mounttags: Option<crate::backend::custom::MountTargets>,
    pub postinstall: Vec<crate::backend::postinstall::Module>,
    pub encrypt: bool,
    pub tpm: bool,
    pub encryption_key: Option<String>,
    pub distro_name: String,
    pub bootc_imgref: Option<String>,
    pub bootc_target_imgref: Option<String>,
    pub bootc_enforce_sigpolicy: bool,
    pub bootc_kargs: Option<Vec<String>>,
    pub bootc_args: Option<Vec<String>>,
}

/// The finalized state of [`InstallationState`].
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FinalInstallationState {
    pub langlocale: String,
    pub destination_disk: DiskInit,
    pub installation_type: DetailedInstallationType,
    pub encrypts: Option<EncryptState>,
    pub config: crate::cfg::ReadymadeConfig,
    pub copy_mode: DetailedCopyMode,
}

#[derive(Default, Serialize, Deserialize, Clone, derivative::Derivative)]
#[derivative(Debug)]
pub struct EncryptState {
    pub tpm: bool,
    #[derivative(Debug = "ignore")]
    pub encryption_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DetailedInstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom {
        mounttags: crate::backend::custom::MountTargets,
    },
}

impl From<&DetailedInstallationType> for InstallationType {
    fn from(value: &DetailedInstallationType) -> Self {
        match value {
            DetailedInstallationType::WholeDisk => Self::WholeDisk,
            DetailedInstallationType::DualBoot(i) => Self::DualBoot(*i),
            DetailedInstallationType::ChromebookInstall => Self::ChromebookInstall,
            DetailedInstallationType::Custom { .. } => Self::Custom,
        }
    }
}

impl DetailedInstallationType {
    fn simple(&self) -> InstallationType {
        self.into()
    }

    /// Returns `true` if the detailed installation type is [`WholeDisk`].
    ///
    /// [`WholeDisk`]: DetailedInstallationType::WholeDisk
    #[must_use]
    pub const fn is_whole_disk(&self) -> bool {
        matches!(self, Self::WholeDisk)
    }

    /// Returns `true` if the detailed installation type is [`DualBoot`].
    ///
    /// [`DualBoot`]: DetailedInstallationType::DualBoot
    #[must_use]
    pub const fn is_dual_boot(&self) -> bool {
        matches!(self, Self::DualBoot(..))
    }

    /// Returns `true` if the detailed installation type is [`ChromebookInstall`].
    ///
    /// [`ChromebookInstall`]: DetailedInstallationType::ChromebookInstall
    #[must_use]
    pub const fn is_chromebook_install(&self) -> bool {
        matches!(self, Self::ChromebookInstall)
    }

    pub const fn as_dual_boot(&self) -> Option<&u64> {
        if let Self::DualBoot(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub const fn mounttags(&self) -> Option<&crate::backend::custom::MountTargets> {
        if let Self::Custom { mounttags } = self {
            Some(mounttags)
        } else {
            None
        }
    }

    /// Returns `true` if the detailed installation type is [`Custom`].
    ///
    /// [`Custom`]: DetailedInstallationType::Custom
    #[must_use]
    pub const fn is_custom(&self) -> bool {
        matches!(self, Self::Custom { .. })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub enum DetailedCopyMode {
    #[default]
    Repart,
    Bootc {
        bootc_imgref: String,
        bootc_target_imgref: Option<String>,
        bootc_enforce_sigpolicy: bool,
        bootc_kargs: Vec<String>,
        bootc_args: Vec<String>,
    },
}

impl DetailedCopyMode {
    /// Returns `true` if the detailed copy mode is [`Repart`].
    ///
    /// [`Repart`]: DetailedCopyMode::Repart
    #[must_use]
    pub const fn is_repart(&self) -> bool {
        matches!(self, Self::Repart)
    }

    /// Returns `true` if the detailed copy mode is [`Bootc`].
    ///
    /// [`Bootc`]: DetailedCopyMode::Bootc
    #[must_use]
    pub const fn is_bootc(&self) -> bool {
        matches!(self, Self::Bootc { .. })
    }
}

impl From<&crate::cfg::ReadymadeConfig> for InstallationState {
    fn from(value: &crate::cfg::ReadymadeConfig) -> Self {
        Self {
            postinstall: value.postinstall.clone(),
            distro_name: value.distro.name.clone(),
            bootc_imgref: value.to_bootc_copy_source(),
            bootc_target_imgref: value.to_bootc_target_copy_source(),
            bootc_enforce_sigpolicy: value.install.bootc_enforce_sigpolicy,
            bootc_kargs: value.install.bootc_kargs.clone(),
            bootc_args: value.install.bootc_args.clone(),
            ..Self::default()
        }
    }
}

impl InstallationState {
    fn to_detailed_installation_type(&self) -> DetailedInstallationType {
        match self
            .installation_type
            .as_ref()
            .expect("A valid installation type should be set before calling install()")
        {
            InstallationType::WholeDisk => DetailedInstallationType::WholeDisk,
            InstallationType::DualBoot(i) => DetailedInstallationType::DualBoot(*i),
            InstallationType::ChromebookInstall => DetailedInstallationType::ChromebookInstall,
            InstallationType::Custom => DetailedInstallationType::Custom {
                mounttags: self
                    .mounttags
                    .clone()
                    .expect("no mounttags needed for installation_type: custom"),
            },
        }
    }
    #[allow(clippy::unwrap_in_result)]
    fn to_encrypt_state(&self) -> Option<EncryptState> {
        self.encrypt.then(|| EncryptState {
            tpm: self.tpm,
            encryption_key: self
                .encryption_key
                .clone()
                .expect("no encryption_key but encrypt = true"),
        })
    }
    // FIX: this is terrible why is copy_mode not passed
    fn to_detailed_copy_mode(&self) -> DetailedCopyMode {
        (self.bootc_imgref.as_ref()).map_or(DetailedCopyMode::Repart, |bootc_imgref| {
            DetailedCopyMode::Bootc {
                bootc_imgref: bootc_imgref.clone(),
                bootc_target_imgref: self.bootc_target_imgref.clone(),
                bootc_enforce_sigpolicy: self.bootc_enforce_sigpolicy,
                bootc_kargs: self.bootc_kargs.clone().unwrap_or_default(),
                bootc_args: self.bootc_args.clone().unwrap_or_default(),
            }
        })
    }
}

impl From<&InstallationState> for FinalInstallationState {
    fn from(value: &InstallationState) -> Self {
        let langlocale = value.langlocale.clone().unwrap_or_else(|| {
            tracing::warn!("why is there no langlocale when generate FinalInstallationState");
            "C.UTF-8".into()
        });
        Self {
            langlocale,
            destination_disk: value.destination_disk.clone().expect("no destination_disk"),
            installation_type: value.to_detailed_installation_type(),
            encrypts: value.to_encrypt_state(),
            config: crate::CONFIG.read().clone(),
            copy_mode: value.to_detailed_copy_mode(),
        }
    }
}

/// IPC installation message for non-interactive mode
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum InstallationMessage {
    Status(String),
}

type CallSubprocessRes = Result<(String, std::io::Result<std::process::Output>)>;

impl FinalInstallationState {
    // todo: move methods from installationstate to here!
    pub fn install_using_subprocess(
        &self,
        sender: &relm4::Sender<InstallationMessage>,
    ) -> Result<()> {
        // HACK: #59 necessitates a way to check for this error message to rerun subprocess:
        const REPART_DEVICE_BUSY: &[u8] =
            b"Failed to reread partition table: Device or resource busy\n";

        if cfg!(debug_assertions) {
            let installation_state_dump_path =
                std::env::temp_dir().join("readymade-installation-state.json");
            tracing::debug!("Dumping installation state to {installation_state_dump_path:?}");
            std::fs::write(installation_state_dump_path, serde_json::to_string(self)?)?;
        }

        let mut retries = 0;
        loop {
            let (logs, res) = self.call_subprocess(sender)?;

            return match res {
                Ok(output) if output.status.success() => Ok(()),
                // PERF: let's only take the last 1024 bytes
                Ok(output)
                    if (logs.as_bytes().last_chunk())
                        .map_or(logs.as_bytes(), |chunk: &[u8; 1024]| chunk)
                        .windows(REPART_DEVICE_BUSY.len())
                        .any(|w| w == REPART_DEVICE_BUSY) =>
                {
                    retries += 1;
                    if retries >= 3 {
                        return Self::subprocess_err(&output, &logs)
                            .warning("Readymade detected that repart errored due to device/resource busy and retried 3 times already");
                    }
                    continue;
                }
                Ok(output) => Self::subprocess_err(&output, &logs),
                Err(e) => Err(eyre!("Failed to execute readymade non-interactively").wrap_err(e)),
            };
        }
    }

    #[allow(clippy::unwrap_in_result)]
    fn call_subprocess(&self, sender: &relm4::Sender<InstallationMessage>) -> CallSubprocessRes {
        let (server, channel_id) = IpcOneShotServer::new()?;
        let handle_ipc = || {
            let (receiver, _) = server.accept().expect("cannot accept ipc server");

            let mut msg;
            while {
                msg = receiver.recv().map(|msg| sender.emit(msg));
                msg.is_ok()
            } {}
            _ = msg.map_err(|e| match e {
                IpcError::Disconnected => {}
                e => tracing::error!("Failed to receive message from subprocess: {e:?}"),
            });
        };
        let mut res = Command::new("pkexec")
            // #93
            .args(["systemd-inhibit", "--who=Readymade", "--why=Installing OS"])
            .arg(std::env::current_exe()?)
            .args(["--non-interactive", &channel_id])
            .args(
                std::env::vars()
                    .filter(|(key, _)| key.starts_with("REPART_") || key.starts_with("READYMADE_"))
                    .map(|(key, value)| format!("{key}={value}")),
            )
            .arg("NO_COLOR=1")
            .arg(format!(
                "READYMADE_LOG={}",
                std::env::var("READYMADE_LOG").as_deref().unwrap_or("info")
            ))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        let child_stdin = res.stdin.as_mut().expect("can't take stdin");
        child_stdin.write_all(serde_json::to_string(self)?.as_bytes())?;
        child_stdin.flush()?;
        let (res, logs) = util::cmd::pipe_cmd("Readymade subprocess logs", res, [handle_ipc]);
        Ok((logs, res))
    }

    fn subprocess_err(output: &std::process::Output, logs: &str) -> Result<()> {
        Err(eyre!("Readymade subprocess failed")
            .with_note(|| output.status.to_string())
            .with_note(|| format!("Logs:\n{logs}")))
    }

    /// Copies the current config into a temporary directory, allowing them to be modified without
    /// affecting the original templates :D
    fn layer_configdir(cfg_dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all("/run/readymade-install")?;
        util::fs::copy_dir(cfg_dir, "/run/readymade-install")?;
        Ok("/run/readymade-install".into())
    }

    #[allow(clippy::unwrap_in_result)]
    #[tracing::instrument]
    pub fn install(&self) -> Result<()> {
        if let DetailedInstallationType::Custom { mut mounttags } = self.installation_type.clone() {
            return crate::backend::custom::install_custom(self, &mut mounttags);
        }
        let blockdev = &self.destination_disk.devpath;

        let inst_type = &self.installation_type;
        tracing::info!("Layering repart templates");
        let cfgdir = Self::layer_configdir(&inst_type.simple().cfgdir(self.copy_mode.is_bootc()))?;

        // Let's write the encryption key to the keyfile
        let keyfile = std::path::Path::new(consts::LUKS_KEYFILE_PATH);
        if let Some(key) = &self.encrypts.as_ref().map(|e| &*e.encryption_key) {
            std::fs::write(keyfile, key)?;
        }

        self.enable_encryption(&cfgdir)?;
        let repart_out = stage!(mkpart {
            // todo: not freeze on error, show error message as err handler?
            Self::systemd_repart(blockdev, &cfgdir, self.encrypts.is_some(), self.copy_mode.is_bootc())?
        });

        let repartcfg_export = super::export::SystemdRepartData::get_configs(&cfgdir)?;

        if self.copy_mode.is_bootc() {
            let tmproot = tempfile::tempdir()?;
            let bootc_rootfs_mountpoint = tmproot.path();
            Self::bootc_mount(
                bootc_rootfs_mountpoint,
                &repart_out,
                self.encrypts.as_ref().map(|e| &*e.encryption_key),
            )?;
            self.bootc_copy(bootc_rootfs_mountpoint, repart_out.generate_cryptdata()?)?;
            crate::cmd!("umount" [["-R"], [bootc_rootfs_mountpoint]]
                => |r| tracing::warn!(rc=?r.code(), ?bootc_rootfs_mountpoint, "cannot umount"));
        }

        tracing::info!("Copying files done, Setting up system...");
        self.setup_system(
            &repart_out,
            self.encrypts.as_ref().map(|e| &*e.encryption_key),
            Some(repartcfg_export),
        )?;

        if self.copy_mode.is_bootc() {
            // Cleanup mount files from bootc thing
            let tmproot = tempfile::tempdir()?;
            let tmproot = tmproot.path();
            Self::bootc_mount(
                tmproot,
                &repart_out,
                self.encrypts.as_ref().map(|e| &*e.encryption_key),
            )?;
            Self::bootc_cleanup(tmproot)?;
            crate::cmd!("sync" => |_| bail!("`sync` failed"));
            crate::cmd!("umount" [["-R"], [tmproot]] => |_| bail!("umount -R {tmproot:?} failed"));
        }

        if let DetailedInstallationType::ChromebookInstall = inst_type {
            // FIXME: don't dd?
            Self::flash_submarine(blockdev)?;
            InstallationType::set_cgpt_flags(blockdev)?;
        }

        tracing::info!("Cleaning up state...");

        if self.encrypts.is_some() {
            std::fs::remove_file(keyfile) // don't care if it fails
                .unwrap_or_else(|e| tracing::warn!("Failed to remove keyfile: {e}"));

            // Close all mapped LUKS devices if exists

            if let Some(mut cache) = super::repart_output::MAPPER_CACHE.try_write() {
                if let Some(cache) = std::sync::Arc::get_mut(&mut cache) {
                    cache.clear();
                }
            }
        }

        tracing::info!("install() finished");
        Ok(())
    }

    // This cleans up any folder that is not on the bootc whitelist from a bootc-installed filesystem
    fn bootc_cleanup(mountpoint: &Path) -> Result<()> {
        _ = std::fs::read_dir(mountpoint)?.try_for_each(|f| {
            let f = f?;
            match f.file_name().as_encoded_bytes() {
                b"boot" | b"ostree" | b"efi" | b".bootc-aleph.json" => {}
                _ => {
                    _ = if f.file_type()?.is_dir() {
                        std::fs::remove_dir_all(f.path())
                    } else {
                        std::fs::remove_file(f.path())
                    }
                }
            }
            std::io::Result::Ok(())
        });
        Ok(())
    }

    // Recursively mounts a bootc-formatted filesystem
    #[allow(clippy::unwrap_in_result)]
    fn bootc_mount(
        targetroot: &Path,
        output: &RepartOutput,
        passphrase: Option<&str>,
    ) -> Result<()> {
        let mut decrypted_partitions: std::collections::HashMap<String, PathBuf> =
            std::collections::HashMap::new();
        let mps = output.mountpoints().map(|(mntpoint, node)| {
            let mp = PathBuf::from(mntpoint);
            if !crate::backend::repart_output::is_luks(&node) {
                return Result::<_, color_eyre::eyre::Error>::Ok((PathBuf::from(node), mp));
            }
            let node = if let Some(mapper) = decrypted_partitions.get(&node) {
                mapper.clone()
            } else {
                let pass = passphrase.ok_or_eyre("Passphrase is empty when is_luks() is true")?;
                // We need to sanitize the label for the mapper device name, as it can't contain slashes
                //
                // I forgot to account for this when I refactored it -Cappy
                //
                let label = crate::backend::repart_output::generate_unique_mapper_label(mntpoint);
                // XXX: This introduces some weird ordering issues with generate_fstab when decrypting from here
                // Because generate_fstab() assumes that the partitions are decrypted already.
                //
                // todo: add some global cache for decrypted partitions
                let mapper = crate::backend::repart_output::luks_decrypt(&node, pass, &label)?;
                decrypted_partitions.insert(node.clone(), mapper.clone());
                mapper
            };
            Result::<_, color_eyre::eyre::Error>::Ok((node, mp))
        });
        let mps = mps.try_collect::<_, Vec<_>, _>()?.into_iter();
        let mut mps = mps.sorted_by(|(_, a), (_, b)| {
            match (a.components().count(), b.components().count()) {
                (1, _) if a.components().next() == Some(std::path::Component::RootDir) => {
                    std::cmp::Ordering::Less
                } // root dir
                (_, 1) if b.components().next() == Some(std::path::Component::RootDir) => {
                    std::cmp::Ordering::Greater
                } // root dir
                (x, y) if x == y => a.cmp(b),
                (x, y) => x.cmp(&y),
            }
        });
        mps.try_for_each(|(source, mntpoint)| {
            let target = targetroot.join(mntpoint.strip_prefix("/").expect("cannot strip /"));
            tracing::debug!(?source, ?target, "mounting");
            std::fs::create_dir_all(&target)?;
            // use shell to mount manually since sys_mount is buggy
            crate::cmd!("mount" [[&source, &target]] =>
                |cmd| bail!("`mount {source:?} → {target:?}` failed with rc: {:?}", cmd.code()));
            Ok(())
        })
    }

    /// Call bootc to copy the contents of the container into the target.
    ///
    /// The caller must verify that `self.copy_mode.is_bootc()`.
    #[allow(clippy::unwrap_in_result, clippy::needless_pass_by_value)]
    pub fn bootc_copy(&self, target_root: &Path, cryptdata: Option<CryptData>) -> Result<()> {
        let DetailedCopyMode::Bootc {
            bootc_imgref: imgref,
            bootc_target_imgref,
            bootc_enforce_sigpolicy,
            bootc_kargs,
            bootc_args,
        } = &self.copy_mode
        else {
            unreachable!()
        };

        tracing::info!(?imgref, "running bootc install to-filesystem");

        crate::cmd!("bootc" [
            ["install", "to-filesystem", "--source-imgref", imgref],
            (cryptdata.iter())
                .flat_map(|data| data.cmdline_opts.iter().flat_map(|opt| ["--karg", opt])),
            ["--karg=rhgb", "--karg=quiet", "--karg=splash"],
            [target_root],
            (bootc_target_imgref.iter()).flat_map(|a| ["--target-imgref", a]),
            bootc_kargs.iter().flat_map(|e| ["--karg", e]),
            bootc_enforce_sigpolicy.then_some("--enforce-container-sigpolicy"),
            bootc_args.iter(),
        ] => |cmd| bail!("`bootc install to-filesystem` failed: {:?}", cmd.code()));

        Ok(())
    }

    #[tracing::instrument]
    fn setup_system(
        &self,
        output: &RepartOutput,
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
        let esp = check_uefi().then(|| output.get_esp_partition()).flatten();
        let xbootldr = output
            .get_xbootldr_partition()
            .context("No xbootldr partition found")?;

        let cryptdata = output.generate_cryptdata()?;

        let rdm_result = super::export::ReadymadeResult::new(self.clone(), repart_cfgs);

        container.run(|| self._inner_sys_setup(fstab, cryptdata, esp, &xbootldr, rdm_result))??;

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
            destination_disk: self.destination_disk.devpath.clone(),
            uefi: util::sys::check_uefi(),
            esp_partition: esp_node,
            xbootldr_partition: xbootldr_node.to_owned(),
            lang: self.langlocale.clone(),
            crypt_data: crypt_data.clone(),
            distro_name: self.config.distro.name.clone(),
        };

        if state_dump.state.copy_mode.is_repart() {
            tracing::info!("Writing /etc/fstab...");
            std::fs::create_dir_all("/etc/").wrap_err("cannot create /etc/")?;
            std::fs::write("/etc/fstab", fstab).wrap_err("cannot write to /etc/fstab")?;
        }

        // Write the state dump to the chroot
        let state_dump_path = Path::new(crate::consts::READYMADE_STATE_PATH);
        let parent =
            (state_dump_path.parent()).context("Invalid state dump path - no parent directory")?;
        std::fs::create_dir_all(parent)
            .wrap_err("Failed to create parent directories for state dump")?;
        std::fs::write(
            state_dump_path,
            state_dump
                .export_string()
                .wrap_err("Failed to serialize state dump")?,
        )
        .wrap_err("Failed to write state dump file")?;

        if let Some(data) = crypt_data.filter(|_| state_dump.state.copy_mode.is_repart()) {
            tracing::info!("Writing /etc/crypttab...");
            std::fs::write("/etc/crypttab", data.crypttab)
                .wrap_err("cannot write to /etc/crypttab")?;
        }

        for module in &self.config.postinstall {
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
                "Using {ROOTFS_BASE} as copy source, as it exists presumably due to raw rootfs in dracut"
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
        let Some(EncryptState { tpm, .. }) = &self.encrypts else {
            return Ok(());
        };
        let root_file = cfgdir.join("50-root.conf");
        let f = std::fs::read_to_string(&root_file)?;
        let f = Self::set_encrypt_to_file(&f, *tpm);
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
        is_bootc: bool,
    ) -> Result<crate::backend::repart_output::RepartOutput> {
        let copy_source = Self::determine_copy_source();
        let dry_run =
            std::env::var("READYMADE_DRY_RUN").map_or(cfg!(debug_assertions), |v| v == "1");
        tracing::debug!(?dry_run, "Running systemd-repart");
        let arg_keyfile = use_keyfile.then(|| {
            let keyfile_path = consts::LUKS_KEYFILE_PATH;
            tracing::debug!("Using keyfile for systemd-repart: {keyfile_path}");
            ["--key-file", keyfile_path]
        });

        let repart_cmd = Command::new("systemd-repart")
            .args(["--dry-run", if dry_run { "yes" } else { "no" }])
            .args(["--definitions", cfgdir.to_str().unwrap()])
            .args(["--empty", "force", "--offline", "false", "--json", "pretty"])
            .args(["--copy-source", &copy_source].iter().filter(|_| is_bootc))
            .args(arg_keyfile.iter().flatten())
            .arg(blockdev)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .context("can't run systemd-repart")?;

        if !repart_cmd.status.success() {
            bail!(
                "systemd-repart errored with status code {:?}",
                repart_cmd.status.code()
            );
        }

        // Dump systemd-repart output to a file if in debug mode
        if cfg!(debug_assertions) {
            let repart_out_path = std::env::temp_dir().join("readymade-repart-output.json");
            tracing::debug!("Dumping systemd-repart output to {repart_out_path:?}");
            std::fs::write(repart_out_path, &repart_cmd.stdout)?;
        }

        // todo: wait for systemd 256 or genfstab magic
        tracing::debug!("systemd-repart finished");
        Ok(serde_json::from_slice(&repart_cmd.stdout)?)
    }
}

impl InstallationType {
    fn cfgdir(&self, is_bootc: bool) -> PathBuf {
        match self {
            Self::ChromebookInstall => repart_dir().join("chromebook"),
            Self::WholeDisk if is_bootc => repart_dir().join("bootcwholedisk"),
            Self::WholeDisk => repart_dir().join("wholedisk"),
            Self::DualBoot(_) => todo!(),
            Self::Custom => unreachable!(),
        }
    }

    fn set_cgpt_flags(blockdev: &Path) -> Result<()> {
        tracing::debug!("Setting cgpt flags");
        let blockdev_str = blockdev.to_str().context("Invalid block device path")?;
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
        let enc =
            super::FinalInstallationState::set_encrypt_to_file("[Partition]\nType=root", false);
        assert!(enc.contains("Encrypt=key-file"),);
    }
}
