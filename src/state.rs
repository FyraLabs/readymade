use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use ipc_channel::{IpcError, ipc::IpcOneShotServer};
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
    /// The user's NVIDIA choice. [`Self::nvidia`] follows it whenever the installation type
    /// offers an NVIDIA image, so a choice forced off by Chromebook comes back afterwards.
    wants_nvidia: bool,
}

impl ApplicationState {
    /// The NVIDIA option starts checked when an NVIDIA display device was detected.
    #[must_use]
    pub fn new(config: &ReadymadeConfig, nvidia_detected: bool) -> Self {
        let mut state = Self {
            wants_nvidia: nvidia_detected,
            ..Self::default()
        };
        state.sync_image_choice(config);
        state
    }

    /// Install modes offered for the NVIDIA choice and installation type.
    #[must_use]
    pub fn image_modes(&self, config: &ReadymadeConfig) -> Vec<ImageMode> {
        self.image_modes_for(config, self.nvidia)
    }

    fn image_modes_for(&self, config: &ReadymadeConfig, nvidia: bool) -> Vec<ImageMode> {
        let mut modes = config.image_modes(nvidia);
        if self.installation_type == Some(InstallationType::ChromebookInstall) {
            modes.retain(|&mode| mode != ImageMode::Mutable);
        }
        modes
    }

    pub fn set_nvidia(&mut self, config: &ReadymadeConfig, nvidia: bool) {
        // The checkbox echoes state changes back here; only a change is the user's choice.
        if nvidia != self.nvidia {
            self.wants_nvidia = nvidia;
        }
        self.sync_image_choice(config);
    }

    pub fn set_installation_type(
        &mut self,
        config: &ReadymadeConfig,
        installation_type: InstallationType,
    ) {
        self.installation_type = Some(installation_type);
        self.sync_image_choice(config);
    }

    /// Turn NVIDIA off while no install mode supports it, and switch install mode if the
    /// current one isn't offered.
    fn sync_image_choice(&mut self, config: &ReadymadeConfig) {
        self.nvidia = self.wants_nvidia && !self.image_modes_for(config, true).is_empty();
        let modes = self.image_modes(config);
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

        let bootc = config.install.copy_mode == CopyMode::Bootc;
        let image = if bootc {
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
            Some(image)
        } else {
            None
        };

        let disk_provisioner = match installation_type {
            InstallationType::Custom => DiskProvisioner::Manual(Manual {
                mounts: custom_mounts
                    .filter(|mounts| !mounts.0.is_empty())
                    .ok_or_eyre(
                        "Custom installation selected but no mount targets were configured",
                    )?,
            }),
            // The bootc repart templates only partition, leaving the copy to the filesystem
            // provisioner.
            InstallationType::WholeDisk | InstallationType::ChromebookInstall => {
                DiskProvisioner::Repart(Repart {
                    directory: installation_type.cfgdir(bootc),
                    copy_source: (!bootc).then(|| PathBuf::from(determine_copy_source())),
                })
            }
            InstallationType::DualBoot(_) => unreachable!(),
        };

        let (filesystem_provisioner, postinstall) = match image {
            None => (
                (installation_type == InstallationType::Custom).then(|| {
                    FileSystemProvisioner::Copy(Copy {
                        copy_source: determine_copy_source(),
                    })
                }),
                config.postinstall.clone(),
            ),
            Some(image) if self.image_mode == ImageMode::Bootc => (
                Some(FileSystemProvisioner::Bootc(Bootc {
                    imgref: image.imgref,
                    target_imgref: image.target_imgref,
                    enforce_sigpolicy: config.install.bootc_enforce_sigpolicy,
                    kargs: config.install.bootc_kargs.clone().unwrap_or_default(),
                    args: config.install.bootc_args.clone().unwrap_or_default(),
                })),
                config.postinstall.clone(),
            ),
            // The Copy provisioner mounts the image with podman and copies its root
            // filesystem; the mutable postinstall modules make it bootable without ostree.
            Some(image) => (
                Some(FileSystemProvisioner::Copy(Copy {
                    copy_source: image.imgref,
                })),
                Self::mutable_postinstall(config),
            ),
        };

        Ok(Playbook {
            destination_disk,
            encryption,
            disk_provisioner,
            filesystem_provisioner,
            postinstall,
        })
    }

    /// The mutable post-install modules, with `install.bootc_kargs` handed to `BootcToMutable`
    /// and `install.nvidia_mok_password` to `NvidiaAkmods`.
    fn mutable_postinstall(config: &ReadymadeConfig) -> Vec<Module> {
        let kargs = config.install.bootc_kargs.as_deref().unwrap_or_default();
        config
            .mutable_postinstall()
            .into_iter()
            .map(|module| match module {
                Module::BootcToMutable(mut m) => {
                    m.kargs.extend_from_slice(kargs);
                    m.into()
                }
                // The UI shows the configured password, so it always wins.
                Module::NvidiaAkmods(mut m) => {
                    m.mok_password
                        .clone_from(&config.install.nvidia_mok_password);
                    m.into()
                }
                module => module,
            })
            .collect()
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
    use libreadymade::backend::postinstall::{
        bootc_to_mutable::BootcToMutable, nvidia_akmods::NvidiaAkmods,
    };
    use libreadymade::disks::Disk;

    fn config() -> ReadymadeConfig {
        crate::cfg::tests::parse(
            r#"
            copy_mode = "bootc"

            [[install.images]]
            imgref = "containers-storage:example/server:44"
            target_imgref = "docker://example/server:44"

            [[install.images]]
            imgref = "containers-storage:example/server-nvidia:44"
            target_imgref = "docker://example/server-nvidia:44"
            nvidia = true
            "#,
        )
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
            wants_nvidia: nvidia,
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
    fn falls_back_to_available_mode() {
        let mut cfg = config();
        let mut state = state(false, ImageMode::Mutable);
        state.set_installation_type(&cfg, InstallationType::ChromebookInstall);
        assert_eq!(state.image_mode, ImageMode::Bootc);
        assert_eq!(state.image_modes(&cfg), vec![ImageMode::Bootc]);

        cfg.install.images[1].modes = vec![ImageMode::Bootc];
        let mut state = self::state(false, ImageMode::Mutable);
        state.set_nvidia(&cfg, true);
        assert_eq!(state.image_mode, ImageMode::Bootc);
    }

    #[test]
    fn chromebook_never_leaves_no_install_mode() {
        let mut cfg = config();
        cfg.install.images[1].modes = vec![ImageMode::Mutable];

        let mut state = state(false, ImageMode::Bootc);
        state.set_installation_type(&cfg, InstallationType::ChromebookInstall);
        state.set_nvidia(&cfg, true);
        assert!(!state.nvidia);
        assert_eq!(state.image_modes(&cfg), [ImageMode::Bootc]);

        // The refused toggle applies once the installation type supports it
        state.set_installation_type(&cfg, InstallationType::WholeDisk);
        assert!(state.nvidia);
        assert_eq!(state.image_mode, ImageMode::Mutable);
        state.set_installation_type(&cfg, InstallationType::ChromebookInstall);
        assert!(!state.nvidia);
        assert_eq!(state.image_mode, ImageMode::Bootc);
    }

    #[test]
    fn chromebook_restores_nvidia() {
        let mut cfg = config();
        cfg.install.images[1].modes = vec![ImageMode::Mutable];
        let mut state = ApplicationState::new(&cfg, true);
        assert!(state.nvidia);

        state.set_installation_type(&cfg, InstallationType::ChromebookInstall);
        assert!(!state.nvidia);
        // the checkbox echoing the forced-off state isn't a choice
        state.set_nvidia(&cfg, false);
        state.set_installation_type(&cfg, InstallationType::ChromebookInstall);
        assert!(!state.nvidia);

        for installation_type in [InstallationType::WholeDisk, InstallationType::Custom] {
            state.set_installation_type(&cfg, InstallationType::ChromebookInstall);
            state.set_installation_type(&cfg, installation_type);
            assert!(state.nvidia);
            assert_eq!(state.image_mode, ImageMode::Mutable);
        }

        // nothing is restored once NVIDIA was turned off by the user
        state.set_nvidia(&cfg, false);
        state.set_installation_type(&cfg, InstallationType::ChromebookInstall);
        state.set_installation_type(&cfg, InstallationType::WholeDisk);
        assert!(!state.nvidia);
    }

    #[test]
    fn chromebook_keeps_supported_nvidia() {
        let cfg = config();
        let mut state = state(true, ImageMode::Bootc);
        state.set_installation_type(&cfg, InstallationType::ChromebookInstall);
        assert!(state.nvidia);
        state.set_installation_type(&cfg, InstallationType::WholeDisk);
        assert!(state.nvidia);
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
    fn mutable_playbook_passes_kargs_and_mok_password() {
        let mut cfg = config();
        cfg.install.bootc_kargs = Some(vec!["console=ttyS0".into()]);
        cfg.install.nvidia_mok_password = "secret".into();
        let playbook = state(true, ImageMode::Mutable)
            .to_playbook(&cfg, None)
            .unwrap();
        let bootc_to_mutable = BootcToMutable {
            kargs: vec!["console=ttyS0".into()],
        };
        let akmods = NvidiaAkmods {
            mok_password: "secret".into(),
        };
        assert!(playbook.postinstall.contains(&bootc_to_mutable.into()));
        assert!(playbook.postinstall.contains(&akmods.into()));
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
        let new = ApplicationState::new(&cfg, true);
        assert!(!new.nvidia);
        assert_eq!(new.image_mode, ImageMode::Bootc);
        let playbook = state(false, ImageMode::Bootc)
            .to_playbook(&cfg, None)
            .unwrap();
        let Some(FileSystemProvisioner::Bootc(bootc)) = playbook.filesystem_provisioner else {
            panic!("expected bootc provisioner");
        };
        assert_eq!(bootc.imgref, "containers-storage:example/legacy:44");
    }
}
