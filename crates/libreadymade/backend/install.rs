use file_guard::Lock;
use ipc_channel::IpcError;
use ipc_channel::ipc::{IpcOneShotServer, IpcSender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

use crate::playbook::Playbook;
use crate::{
    backend::{
        postinstall::PostInstallModule,
        repart_output::{CryptData, RepartOutput},
    },
    consts::{self, LIVE_BASE, ROOTFS_BASE, repart_dir},
    disks::Disk,
    prelude::*,
    stage,
    util::{self, sys::check_uefi},
};

pub static IPC_CHANNEL: OnceLock<Mutex<IpcSender<InstallationMessage>>> = OnceLock::new();

/// IPC installation message for non-interactive mode
#[derive(serde::Serialize)]
pub enum InstallationMessage {
    Status(String),
}

type CallSubprocessRes = Result<(String, std::io::Result<std::process::Output>)>;

impl Playbook {
    /// # Errors
    /// Fails when the subprocess returns non-zero code
    // TODO: move methods from installationstate to here!
    pub fn install_using_subprocess<F: FnMut(InstallationMessage) + std::marker::Send>(
        &self,
        mut sender: F,
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
            let (logs, res) = self.call_subprocess(&mut sender)?;

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
    fn call_subprocess<F: FnMut(InstallationMessage) + std::marker::Send>(
        &self,
        mut sender: F,
    ) -> CallSubprocessRes {
        let (server, channel_id) = IpcOneShotServer::new()?;
        let handle_ipc = || {
            let (receiver, _) = server.accept().expect("cannot accept ipc server");

            let mut msg;
            while {
                msg = receiver.recv().map(|msg| sender(msg));
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
            .arg("RUST_BACKTRACE=full")
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
        tracing::error!(
            status = output.status.to_string(),
            "Readymade subprocess failed"
        );
        tracing::error!(logs);
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
        let repart_out = stage!(mkpart "Creating partitions and copying files" {
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

        tracing::info!("Cleaning up state...");

        if self.encrypts.is_some() {
            std::fs::remove_file(keyfile) // don't care if it fails
                .unwrap_or_else(|e| tracing::warn!("Failed to remove keyfile: {e}"));

            // Close all mapped LUKS devices if exists

            if let Some(mut cache) = super::repart_output::MAPPER_CACHE.try_write()
                && let Some(cache) = std::sync::Arc::get_mut(&mut cache)
            {
                cache.clear();
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
                decrypted_partitions.insert(node, mapper.clone());
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
        // Let's create a lockfile to prevent running _inner_sys_setup outside the chroot jail
        let lockfile_path = "/var/run/readymade-setup.lock";
        std::fs::write(lockfile_path, b"")?;

        // todo: Also handle custom installs? Needs more information
        let esp = check_uefi().then(|| output.get_esp_partition()).flatten();
        let xbootldr = output
            .get_xbootldr_partition()
            .context("No xbootldr partition found")?;

        let cryptdata = output.generate_cryptdata()?;

        let rdm_result = super::export::ReadymadeResult::new(self.clone(), repart_cfgs);

        let tempdir = tempfile::tempdir()?;

        // XXX: This is a bit hacky, but this function should be called before output.generate_fstab() for
        // the fstab generator to be correct, IF we're using encryption
        //
        // todo: Unfuck this
        let mut container = output.to_container(
            &tempdir,
            passphrase,
            !rdm_result.state.copy_mode.is_repart(),
        )?;
        let fstab = output.generate_fstab()?;
        // tiffin will run `nix::unistd::chdir("/")` when entering the container, so we can use `sysroot as above`
        container.run(|| self.inner_sys_setup(fstab, cryptdata, esp, &xbootldr, &rdm_result))??;

        // Let's remove the lockfile now that we're done
        std::fs::remove_file(lockfile_path)
            .wrap_err("Failed to remove setup lock file after installation")?;

        Ok(())
    }

    /// Mount a device or file to /mnt/live-base
    fn mount_dev(dev: &str) -> std::io::Result<sys_mount::Mount> {
        const MOUNTPOINT: &str = "/mnt/live-base";
        std::fs::create_dir_all(MOUNTPOINT)?;
        sys_mount::Mount::builder().mount(dev, MOUNTPOINT)
    }

    #[must_use]
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

        // Scope to ensure device and lock live long enough for the command
        let repart_cmd = {
            // lock device
            let mut device = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(blockdev)
                .context("Failed to open block device")?;
            // We are locking the device so that repart doesn't fail due to device busy
            // The lock is held in this scope
            let mut _lock = file_guard::lock(&mut device, Lock::Exclusive, 0, 1)?;
            let mut cmd = Command::new("systemd-repart");
            // HACK: Disable whole-device TRIM to reduce wear on SSDs and formatting time
            // https://github.com/systemd/systemd/issues/32760
            // TODO: Turn off once systemd 259 lands
            // https://github.com/systemd/systemd/commit/29ee9c6fb7c75c421f887c8579c65eb04d4f634d
            //
            cmd.env("SYSTEMD_REPART_MKFS_OPTIONS_BTRFS", "--nodiscard")
                .args(["--dry-run", if dry_run { "yes" } else { "no" }])
                .args(["--definitions", cfgdir.to_str().unwrap()])
                .args(["--empty", "force", "--offline", "false", "--json", "pretty"])
                .args(["--copy-source", &copy_source].iter().filter(|_| !is_bootc))
                .args(arg_keyfile.iter().flatten())
                .arg(blockdev)
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit());

            tracing::debug!(?cmd, "Executing systemd-repart command");

            cmd.output().context("can't run systemd-repart")?
        };

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
            Self::ChromebookInstall => repart_dir().join("chromebookinstall"),
            Self::WholeDisk if is_bootc => repart_dir().join("bootcwholedisk"),
            Self::WholeDisk => repart_dir().join("wholedisk"),
            Self::DualBoot(_) => todo!(),
            Self::Custom => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_set_encrypt_to_file() {
        let enc =
            super::FinalInstallationState::set_encrypt_to_file("[Partition]\nType=root", false);
        assert!(enc.contains("Encrypt=key-file"));
    }
}
