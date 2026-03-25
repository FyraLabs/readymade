use std::{
    path::{Path, PathBuf},
    process::Command,
};

use libreadymade::{
    backend::{
        mounts::{Mount, Mounts},
        provisioners::{
            DiskProvisioner, FileSystemProvisioner,
            disk::{manual::Manual, repart::Repart},
            filesystem::{Bootc, Copy},
        },
    },
    consts::{LIVE_BASE, ROOTFS_BASE},
    disks::Disk,
    playbook::{EncryptionConfig, Playbook},
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
                            copy_source: PathBuf::from(determine_copy_source()),
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

        Ok(Playbook {
            destination_disk,
            encryption,
            disk_provisioner,
            filesystem_provisioner,
            postinstall: config.postinstall.clone(),
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

pub fn determine_copy_source() -> String {
    const FALLBACK: &str = "/mnt/live-base";

    std::env::var("REPART_COPY_SOURCE").unwrap_or_else(|_| {
        if std::fs::metadata(ROOTFS_BASE)
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            tracing::info!("Using {ROOTFS_BASE} as copy source");
            ROOTFS_BASE.to_owned()
        } else {
            match mount_live_base(LIVE_BASE) {
                Ok(path) => {
                    let path = path.to_string_lossy().to_string();
                    tracing::info!("Mounted live-base at {path}");
                    path
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to mount `{LIVE_BASE}`, using `{FALLBACK}` as copy source anyway... ({err})"
                    );
                    FALLBACK.to_owned()
                }
            }
        }
    })
}

fn mount_live_base(dev: &str) -> std::io::Result<PathBuf> {
    const MOUNTPOINT: &str = "/mnt/live-base";
    std::fs::create_dir_all(MOUNTPOINT)?;
    let status = Command::new("mount").arg(dev).arg(MOUNTPOINT).status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "mount returned status {status}"
        )));
    }
    Ok(Path::new(MOUNTPOINT).to_path_buf())
}

/// State related to the user's installation configuration
pub static APPLICATION_STATE: SharedState<ApplicationState> = SharedState::new();
