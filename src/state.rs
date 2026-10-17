use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use ipc_channel::{IpcError, ipc::IpcOneShotServer};
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
    playbook::{EncryptionConfig, Playbook, PlaybookProgress},
};
use relm4::SharedState;

use crate::cfg::{CopyMode, ImageMode, InstallationType, ReadymadeConfig};
use crate::prelude::*;

#[derive(Default)]
pub struct ApplicationState {
    pub destination_disk: Option<Disk>,
    pub encrypt: bool,
    pub encryption_key: Option<String>,
    pub tpm: bool,
    pub lang: Option<String>,
    pub installation_type: Option<InstallationType>,
    /// Install the NVIDIA variant of the image.
    pub nvidia: bool,
    pub image_mode: ImageMode,
}

impl ApplicationState {
    /// Initial state for `config`. The NVIDIA option starts checked when an NVIDIA display device
    /// was detected and the config offers an NVIDIA image.
    #[must_use]
    pub fn new(config: &ReadymadeConfig, nvidia_detected: bool) -> Self {
        let nvidia = nvidia_detected && !config.image_modes(true).is_empty();
        Self {
            nvidia,
            image_mode: config
                .image_modes(nvidia)
                .first()
                .copied()
                .unwrap_or_default(),
            ..Self::default()
        }
    }

    /// Toggle the NVIDIA option, switching install mode if the current one has no matching image.
    pub fn set_nvidia(&mut self, config: &ReadymadeConfig, nvidia: bool) {
        self.nvidia = nvidia;
        let modes = config.image_modes(nvidia);
        if !modes.contains(&self.image_mode)
            && let Some(&mode) = modes.first()
        {
            self.image_mode = mode;
        }
    }

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

        if config.install.copy_mode == CopyMode::Bootc {
            if config.images().is_empty() {
                bail!("Bootc copy mode selected but no source image reference is configured");
            }
            let image = config
                .select_image(self.nvidia, self.image_mode)
                .ok_or_eyre("No configured image matches the selected NVIDIA and install mode")?;
            if self.image_mode == ImageMode::Mutable
                && installation_type == InstallationType::ChromebookInstall
            {
                bail!("Mutable installs are not supported for Chromebook installs");
            }
            let disk_provisioner = Self::image_disk_provisioner(installation_type, custom_mounts)?;
            let (filesystem_provisioner, postinstall) = match self.image_mode {
                ImageMode::Bootc => (
                    FileSystemProvisioner::Bootc(Bootc {
                        imgref: image.imgref,
                        target_imgref: image.target_imgref,
                        enforce_sigpolicy: config.install.bootc_enforce_sigpolicy,
                        kargs: config.install.bootc_kargs.clone().unwrap_or_default(),
                        args: config.install.bootc_args.clone().unwrap_or_default(),
                    }),
                    config.postinstall.clone(),
                ),
                // The Copy provisioner mounts the image with podman and copies its root
                // filesystem; the mutable postinstall modules make it bootable without ostree.
                ImageMode::Mutable => (
                    FileSystemProvisioner::Copy(Copy {
                        copy_source: image.imgref,
                    }),
                    config.mutable_postinstall(),
                ),
            };
            return Ok(Playbook {
                destination_disk,
                encryption,
                disk_provisioner,
                filesystem_provisioner: Some(filesystem_provisioner),
                postinstall,
            });
        }

        let (disk_provisioner, filesystem_provisioner) = match installation_type {
            InstallationType::Custom => (
                DiskProvisioner::Manual(Manual {
                    mounts: Self::custom_mounts(custom_mounts)?,
                }),
                Some(FileSystemProvisioner::Copy(Copy {
                    copy_source: determine_copy_source(),
                })),
            ),
            InstallationType::WholeDisk | InstallationType::ChromebookInstall => (
                DiskProvisioner::Repart(Repart {
                    directory: installation_type.cfgdir(false),
                    copy_source: Some(PathBuf::from(determine_copy_source())),
                }),
                None,
            ),
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

    /// Disk provisioner for image installs. Whole-disk installs use the bootc repart templates,
    /// which only partition and leave copying files to the filesystem provisioner.
    fn image_disk_provisioner(
        installation_type: InstallationType,
        custom_mounts: Option<Mounts>,
    ) -> Result<DiskProvisioner> {
        Ok(match installation_type {
            InstallationType::Custom => DiskProvisioner::Manual(Manual {
                mounts: Self::custom_mounts(custom_mounts)?,
            }),
            InstallationType::WholeDisk | InstallationType::ChromebookInstall => {
                DiskProvisioner::Repart(Repart {
                    directory: installation_type.cfgdir(true),
                    copy_source: None,
                })
            }
            InstallationType::DualBoot(_) => unreachable!(),
        })
    }

    fn custom_mounts(custom_mounts: Option<Mounts>) -> Result<Mounts> {
        custom_mounts
            .filter(|mounts| !mounts.0.is_empty())
            .ok_or_eyre("Custom installation selected but no mount targets were configured")
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

pub fn install_using_subprocess<F>(playbook: &Playbook, mut on_progress: F) -> Result<()>
where
    F: FnMut(PlaybookProgress) + Send + 'static,
{
    let (server, channel_id) = IpcOneShotServer::new()?;
    let ipc_thread = std::thread::spawn(move || {
        let (receiver, _) = server.accept().expect("cannot accept ipc server");

        let mut msg;
        while {
            msg = receiver.recv().map(&mut on_progress);
            msg.is_ok()
        } {}

        if let Err(err) = msg {
            match err {
                IpcError::Disconnected => {}
                other => tracing::error!("Failed to receive progress from subprocess: {other:?}"),
            }
        }
    });

    let mut child = Command::new("pkexec")
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdin = child
        .stdin
        .as_mut()
        .ok_or_eyre("Failed to open subprocess stdin")?;
    stdin.write_all(serde_json::to_string(playbook)?.as_bytes())?;
    stdin.flush()?;

    let output = child.wait_with_output()?;
    ipc_thread.join().expect("cannot join ipc thread");
    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(eyre!("Readymade subprocess failed"))
        .with_note(|| output.status.to_string())
        .with_note(|| format!("stdout:\n{stdout}"))
        .with_note(|| format!("stderr:\n{stderr}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use libreadymade::disks::Disk;
    use serde_valid::toml::FromTomlStr;

    const CONFIG: &str = r#"
        postinstall = [{ module = "Script" }]

        [distro]
        name = "Test"

        [install]
        allowed_installtypes = ["wholedisk", "custom"]
        copy_mode = "bootc"

        [[install.images]]
        imgref = "containers-storage:example/server:44"
        target_imgref = "docker://example/server:44"

        [[install.images]]
        imgref = "containers-storage:example/server-nvidia:44"
        target_imgref = "docker://example/server-nvidia:44"
        nvidia = true

        [[bento]]
        title = "a"
        desc = "a"
        link = "a"
        icon = "a"

        [[bento]]
        title = "b"
        desc = "b"
        link = "b"
        icon = "b"

        [[bento]]
        title = "c"
        desc = "c"
        link = "c"
        icon = "c"
    "#;

    fn config() -> ReadymadeConfig {
        ReadymadeConfig::from_toml_str(CONFIG).unwrap()
    }

    fn state(nvidia: bool, image_mode: ImageMode) -> ApplicationState {
        ApplicationState {
            destination_disk: Some(Disk {
                disk_name: "vda".into(),
                os_name: None,
                devpath: PathBuf::from("/dev/vda"),
                size: Default::default(),
            }),
            installation_type: Some(InstallationType::WholeDisk),
            nvidia,
            image_mode,
            ..ApplicationState::default()
        }
    }

    #[test]
    fn nvidia_preselected_only_when_detected_and_available() {
        let cfg = config();
        assert!(ApplicationState::new(&cfg, true).nvidia);
        assert!(!ApplicationState::new(&cfg, false).nvidia);

        let mut no_nvidia = cfg;
        no_nvidia.install.images.retain(|img| !img.nvidia);
        assert!(!ApplicationState::new(&no_nvidia, true).nvidia);
    }

    #[test]
    fn set_nvidia_falls_back_to_available_mode() {
        let mut cfg = config();
        cfg.install.images[1].modes = vec![ImageMode::Bootc];
        let mut state = state(false, ImageMode::Mutable);
        state.set_nvidia(&cfg, true);
        assert_eq!(state.image_mode, ImageMode::Bootc);
    }

    #[test]
    fn bootc_playbook_uses_selected_image() {
        let cfg = config();
        let playbook = state(true, ImageMode::Bootc)
            .to_playbook(&cfg, None)
            .unwrap();
        let Some(FileSystemProvisioner::Bootc(bootc)) = playbook.filesystem_provisioner else {
            panic!("expected bootc provisioner");
        };
        assert_eq!(bootc.imgref, "containers-storage:example/server-nvidia:44");
        assert_eq!(
            bootc.target_imgref.as_deref(),
            Some("docker://example/server-nvidia:44")
        );
        assert_eq!(playbook.postinstall, cfg.postinstall);
    }

    #[test]
    fn mutable_playbook_copies_image_without_repart_copy() {
        let cfg = config();
        let playbook = state(false, ImageMode::Mutable)
            .to_playbook(&cfg, None)
            .unwrap();
        let DiskProvisioner::Repart(repart) = &playbook.disk_provisioner else {
            panic!("expected repart");
        };
        assert_eq!(repart.copy_source, None);
        assert_eq!(repart.directory, InstallationType::WholeDisk.cfgdir(true));
        assert_eq!(
            playbook.filesystem_provisioner,
            Some(FileSystemProvisioner::Copy(Copy {
                copy_source: "containers-storage:example/server:44".into()
            }))
        );
        assert_eq!(playbook.postinstall, cfg.mutable_postinstall());
    }

    #[test]
    fn mutable_chromebook_is_rejected() {
        let mut state = state(false, ImageMode::Mutable);
        state.installation_type = Some(InstallationType::ChromebookInstall);
        assert!(state.to_playbook(&config(), None).is_err());
    }

    #[test]
    fn legacy_bootc_config_still_works() {
        let mut cfg = config();
        cfg.install.images.clear();
        cfg.install.bootc_imgref = Some("containers-storage:example/legacy:44".into());
        let state = ApplicationState::new(&cfg, true);
        assert!(!state.nvidia);
        assert_eq!(state.image_mode, ImageMode::Bootc);
        let playbook = ApplicationState {
            destination_disk: self::state(false, ImageMode::Bootc).destination_disk,
            installation_type: Some(InstallationType::WholeDisk),
            ..state
        }
        .to_playbook(&cfg, None)
        .unwrap();
        let Some(FileSystemProvisioner::Bootc(bootc)) = playbook.filesystem_provisioner else {
            panic!("expected bootc provisioner");
        };
        assert_eq!(bootc.imgref, "containers-storage:example/legacy:44");
    }
}
