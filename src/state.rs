use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use ipc_channel::{
    IpcError,
    ipc::{IpcOneShotServer, IpcSender},
};
use libreadymade::{
    backend::{
        mounts::{Mount, Mounts},
        postinstall::Module,
        provisioners::{
            DiskProvisioner, FileSystemProvisioner,
            disk::{manual::Manual, repart::Repart},
            filesystem::{Bootc, Copy},
        },
    },
    consts::{LIVE_BASE, ROOTFS_BASE},
    disks::Disk,
    playbook::{EncryptionConfig, Playbook, PlaybookProgress},
};
use relm4::SharedState;

use crate::cfg::{CopyMode, InstallationType, ReadymadeConfig};
use crate::prelude::*;

#[derive(Default)]
pub struct ApplicationState {
    pub destination_disk: Option<Disk>,
    pub encrypt: bool,
    pub encryption_key: Option<String>,
    pub tpm: bool,
    /// glibc locale picked on the language page, without the codeset (e.g. `en_US`)
    pub lang: Option<String>,
    pub installation_type: Option<InstallationType>,
}

impl ApplicationState {
    pub fn to_playbook(
        &self,
        config: &ReadymadeConfig,
        custom_mounts: Option<Mounts>,
    ) -> Result<Playbook> {
        let destination_disk = self
            .destination_disk
            .as_ref()
            .ok_or_eyre("No destination disk selected")?
            .devpath
            .clone();
        let installation_type = self
            .installation_type
            .ok_or_eyre("No installation type selected")?;

        if matches!(installation_type, InstallationType::DualBoot(_)) {
            bail!("Dual boot installation is not wired into the new playbook flow yet");
        }

        let encryption = if self.encrypt {
            Some(EncryptionConfig {
                tpm: self.tpm,
                encryption_key: self
                    .encryption_key
                    .clone()
                    .ok_or_eyre("Encryption was enabled but no passphrase was provided")?,
            })
        } else {
            None
        };

        let (disk_provisioner, filesystem_provisioner) = match installation_type {
            InstallationType::Custom => {
                let mounts = custom_mounts
                    .filter(|mounts| !mounts.0.is_empty())
                    .ok_or_eyre(
                        "Custom installation selected but no mount targets were configured",
                    )?;
                (
                    DiskProvisioner::Manual(Manual { mounts }),
                    Some(match config.install.copy_mode {
                        CopyMode::Bootc => FileSystemProvisioner::Bootc(Bootc {
                            imgref: config
                                .to_bootc_copy_source()
                                .ok_or_eyre("Bootc copy mode selected but no source image reference is configured")?,
                            target_imgref: config.to_bootc_target_copy_source(),
                            enforce_sigpolicy: config.install.bootc_enforce_sigpolicy,
                            kargs: config.install.bootc_kargs.clone().unwrap_or_default(),
                            args: config.install.bootc_args.clone().unwrap_or_default(),
                        }),
                        CopyMode::Repart => FileSystemProvisioner::Copy(Copy {
                            copy_source: determine_copy_source(),
                        }),
                    }),
                )
            }
            InstallationType::WholeDisk | InstallationType::ChromebookInstall => match config
                .install
                .copy_mode
            {
                CopyMode::Bootc => (
                    DiskProvisioner::Repart(Repart {
                        directory: installation_type.cfgdir(true),
                        copy_source: None,
                    }),
                    Some(FileSystemProvisioner::Bootc(Bootc {
                        imgref: config.to_bootc_copy_source().ok_or_eyre(
                            "Bootc copy mode selected but no source image reference is configured",
                        )?,
                        target_imgref: config.to_bootc_target_copy_source(),
                        enforce_sigpolicy: config.install.bootc_enforce_sigpolicy,
                        kargs: config.install.bootc_kargs.clone().unwrap_or_default(),
                        args: config.install.bootc_args.clone().unwrap_or_default(),
                    })),
                ),
                CopyMode::Repart => (
                    DiskProvisioner::Repart(Repart {
                        directory: installation_type.cfgdir(false),
                        copy_source: Some(PathBuf::from(determine_copy_source())),
                    }),
                    None,
                ),
            },
            InstallationType::DualBoot(_) => unreachable!(),
        };

        let mut postinstall = config.postinstall.clone();
        if let Some(lang) = &self.lang {
            // write the language picked on the language page, not the template's placeholder
            for module in &mut postinstall {
                if let Module::Language(module) = module {
                    module.lang.clone_from(lang);
                }
            }
        }

        Ok(Playbook {
            destination_disk,
            encryption,
            disk_provisioner,
            filesystem_provisioner,
            postinstall,
        })
    }
}

pub fn mount_from_custom_target(partition: &str, mountpoint: &str, options: &str) -> Mount {
    Mount::new(
        PathBuf::from(partition),
        PathBuf::from(mountpoint),
        options.to_owned(),
        None,
        None,
    )
}

/// Work out what to copy the new system from.
///
/// This runs unprivileged, so it only picks a path: if it's an image or block device (like the
/// live ISO's `live-base`), the playbook mounts it once it's running as root.
pub fn determine_copy_source() -> String {
    std::env::var("REPART_COPY_SOURCE").unwrap_or_else(|_| {
        if std::fs::metadata(ROOTFS_BASE).is_ok_and(|m| m.is_dir()) {
            tracing::info!("Using {ROOTFS_BASE} as copy source");
            ROOTFS_BASE.to_owned()
        } else {
            tracing::info!("Using {LIVE_BASE} as copy source");
            LIVE_BASE.to_owned()
        }
    })
}

/// `pkexec` exits with this when the user dismissed the authentication dialog.
const PKEXEC_DISMISSED: i32 = 126;

/// The user closed the polkit authentication dialog, so the installation never started.
#[derive(Debug)]
pub struct AuthCancelled;

impl std::fmt::Display for AuthCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Authentication was cancelled")
    }
}

impl std::error::Error for AuthCancelled {}

/// If the child never connects, the ipc thread is stuck in `accept()`. Connecting and hanging up
/// ourselves lets it finish. If the child did connect the server is gone and this just fails.
fn hang_up(channel_id: String) {
    drop(IpcSender::<PlaybookProgress>::connect(channel_id));
}

/// Run `playbook` as root in a `pkexec`-ed copy of ourselves, calling `on_progress` (from another
/// thread) for every progress message it reports.
///
/// # Errors
/// - [`AuthCancelled`] if the user dismissed the polkit dialog
/// - the subprocess failed or could not be started
pub fn install_using_subprocess<F>(playbook: &Playbook, mut on_progress: F) -> Result<()>
where
    F: FnMut(PlaybookProgress) + Send + 'static,
{
    let exe = std::env::current_exe()?;
    let (server, channel_id) = IpcOneShotServer::<PlaybookProgress>::new()?;
    let ipc_thread = std::thread::spawn(move || {
        // NOTE: accept() doesn't return until the first message has been received
        let receiver = match server.accept() {
            Ok((receiver, first)) => {
                on_progress(first);
                receiver
            }
            Err(IpcError::Disconnected) => return,
            Err(err) => return tracing::error!(?err, "Failed to accept subprocess connection"),
        };
        loop {
            match receiver.recv() {
                Ok(msg) => on_progress(msg),
                Err(IpcError::Disconnected) => break,
                Err(err) => {
                    tracing::error!(?err, "Failed to receive progress from subprocess");
                    break;
                }
            }
        }
    });

    let child = Command::new("pkexec")
        .args(["systemd-inhibit", "--who=Readymade", "--why=Installing OS"])
        .arg(exe)
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(err) => {
            hang_up(channel_id);
            _ = ipc_thread.join();
            return Err(err).wrap_err("Failed to run pkexec");
        }
    };

    // the child might be gone already (e.g. auth was cancelled), so don't bail before reaping it
    let written = (|| -> Result<()> {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_eyre("Failed to open subprocess stdin")?;
        stdin.write_all(serde_json::to_string(playbook)?.as_bytes())?;
        stdin.flush()?;
        Ok(())
    })();

    let output = child.wait_with_output();
    hang_up(channel_id);
    if ipc_thread.join().is_err() {
        tracing::error!("progress thread panicked");
    }

    let output = output.wrap_err("Failed to wait for the subprocess")?;
    if output.status.code() == Some(PKEXEC_DISMISSED) {
        return Err(AuthCancelled.into());
    }
    if output.status.success() {
        return written.wrap_err("Failed to send the playbook to the subprocess");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let report = eyre!("Readymade subprocess failed")
        .with_note(|| output.status.to_string())
        .with_note(|| format!("stdout:\n{stdout}"))
        .with_note(|| format!("stderr:\n{stderr}"));
    Err(match written {
        Ok(()) => report,
        Err(e) => report.with_note(|| format!("Failed to send the playbook: {e}")),
    })
}

/// State related to the user's installation configuration
pub static APPLICATION_STATE: SharedState<ApplicationState> = SharedState::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hang_up_unblocks_accept() {
        let (server, channel_id) = IpcOneShotServer::<PlaybookProgress>::new().unwrap();
        let th = std::thread::spawn(move || server.accept().map(|_| ()));
        hang_up(channel_id.clone());
        assert!(matches!(th.join().unwrap(), Err(IpcError::Disconnected)));
        // the server is gone now, so this must not block or panic
        hang_up(channel_id);
    }
}
