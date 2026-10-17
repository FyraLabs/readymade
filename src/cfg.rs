#![allow(clippy::str_to_string)]
use crate::prelude::*;
use crate::state::ApplicationState;
use libreadymade::consts::repart_dir;
use serde::{Deserialize, Serialize};
use serde_valid::{Validate, toml::FromTomlStr};

use libreadymade::backend::postinstall::Module;

#[cfg(not(debug_assertions))]
const DEFAULT_CFG_PATH: &str = "/etc/readymade.toml";

#[cfg(debug_assertions)]
const DEFAULT_CFG_PATH: &str = "crates/libreadymade/templates/ultramarine.toml";

#[derive(Deserialize, Serialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CopyMode {
    #[default]
    Repart,
    Bootc,
}

/// How a bootc image is installed onto the target.
#[derive(Deserialize, Serialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageMode {
    /// Image-based install through `bootc install to-filesystem`.
    #[default]
    Bootc,
    /// Traditional, package-managed install: the image's root filesystem is copied to the target
    /// and converted to boot without ostree.
    Mutable,
}

/// A bootc image that can be installed. Used when `copy_mode = "bootc"`.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Source image reference, with a transport prefix (e.g. `containers-storage:` or `docker://`).
    pub imgref: String,
    /// Image reference the installed system tracks for updates (bootc installs only).
    pub target_imgref: Option<String>,
    /// Whether this image ships the NVIDIA driver.
    #[serde(default)]
    pub nvidia: bool,
    /// The install modes this image can be used for.
    #[serde(default = "_default_image_modes")]
    pub modes: Vec<ImageMode>,
}

fn _default_image_modes() -> Vec<ImageMode> {
    vec![ImageMode::Bootc, ImageMode::Mutable]
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

#[derive(Deserialize, Serialize, Validate, Default, Debug, Clone, PartialEq, Eq)]
pub struct Install {
    #[validate(min_items = 1)]
    pub allowed_installtypes: Vec<InstallationType>,
    #[serde(default)]
    pub copy_mode: CopyMode,
    pub bootc_imgref: Option<String>,
    pub bootc_target_imgref: Option<String>,
    #[serde(default)]
    pub bootc_enforce_sigpolicy: bool,
    pub bootc_kargs: Option<Vec<String>>,
    pub bootc_args: Option<Vec<String>>,
    /// Images to choose from at install time. When empty, `bootc_imgref` is used as the only
    /// image, for bootc installs only.
    #[serde(default)]
    pub images: Vec<Image>,
    /// Documentation shown when installing NVIDIA drivers with Secure Boot enabled.
    #[serde(default = "_default_nvidia_secureboot_docs")]
    pub nvidia_secureboot_docs: String,
}

// TODO: placeholder, replace once the page exists
fn _default_nvidia_secureboot_docs() -> String {
    "https://wiki.ultramarine-linux.org/en/usage/nvidia-secure-boot/".into()
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct Distro {
    pub name: String,
    #[serde(default = "_default_icon")]
    pub icon: String,
    #[serde(default)]
    pub bios_support: bool,
}

fn _default_icon() -> String {
    "fedora-logo-icon".into()
}

#[derive(Deserialize, Serialize, Validate, Default, Debug, Clone, PartialEq, Eq)]
pub struct ReadymadeConfig {
    #[serde(default)]
    pub no_langpage: bool,
    pub distro: Distro,
    pub install: Install,
    pub postinstall: Vec<Module>,
    /// Post-install modules for mutable installs from a bootc image. Defaults to
    /// [`default_mutable_postinstall`].
    pub mutable_postinstall: Option<Vec<Module>>,
    #[serde(rename = "bento")]
    pub bentos: [Bento; 3],
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct Bento {
    pub title: String,
    pub desc: String,
    pub link: String,
    pub icon: String,
}

impl ReadymadeConfig {
    #[must_use]
    pub fn to_bootc_copy_source(&self) -> Option<String> {
        let s = self.install.bootc_imgref.clone();
        s.filter(|_| self.install.copy_mode == crate::cfg::CopyMode::Bootc)
            .or_else(|| std::env::var("COPY_SOURCE").ok())
    }
    #[must_use]
    pub fn to_bootc_target_copy_source(&self) -> Option<String> {
        let s = self.install.bootc_target_imgref.clone();
        s.filter(|_| self.install.copy_mode == crate::cfg::CopyMode::Bootc)
            .or_else(|| std::env::var("TARGET_COPY_SOURCE").ok())
    }

    /// The images available for installation.
    ///
    /// Configs without `install.images` get a single bootc-only image built from `bootc_imgref`.
    #[must_use]
    pub fn images(&self) -> Vec<Image> {
        if self.install.copy_mode != CopyMode::Bootc {
            return vec![];
        }
        if !self.install.images.is_empty() {
            return self.install.images.clone();
        }
        self.to_bootc_copy_source()
            .map(|imgref| Image {
                imgref,
                target_imgref: self.to_bootc_target_copy_source(),
                nvidia: false,
                modes: vec![ImageMode::Bootc],
            })
            .into_iter()
            .collect()
    }

    /// Picks the first image matching the NVIDIA choice that supports `mode`.
    #[must_use]
    pub fn select_image(&self, nvidia: bool, mode: ImageMode) -> Option<Image> {
        self.images()
            .into_iter()
            .find(|img| img.nvidia == nvidia && img.modes.contains(&mode))
    }

    /// Install modes offered for the given NVIDIA choice, in a stable order.
    #[must_use]
    pub fn image_modes(&self, nvidia: bool) -> Vec<ImageMode> {
        [ImageMode::Bootc, ImageMode::Mutable]
            .into_iter()
            .filter(|&mode| self.select_image(nvidia, mode).is_some())
            .collect()
    }

    #[must_use]
    pub fn has_nvidia_image(&self) -> bool {
        self.images().iter().any(|img| img.nvidia)
    }

    #[must_use]
    pub fn mutable_postinstall(&self) -> Vec<Module> {
        self.mutable_postinstall
            .clone()
            .unwrap_or_else(default_mutable_postinstall)
    }

    /// # Errors
    /// - `install.images` is set but `copy_mode` is not `bootc`
    /// - an image lists no install modes
    pub fn check(&self) -> Result<()> {
        if !self.install.images.is_empty() && self.install.copy_mode != CopyMode::Bootc {
            bail!("install.images requires install.copy_mode = \"bootc\"");
        }
        if let Some(img) = self.install.images.iter().find(|img| img.modes.is_empty()) {
            bail!("image {:?} must list at least one install mode", img.imgref);
        }
        Ok(())
    }
}

/// Post-install modules used for mutable installs when the config doesn't set
/// `mutable_postinstall`.
#[must_use]
pub fn default_mutable_postinstall() -> Vec<Module> {
    use libreadymade::backend::postinstall::{
        bootc_to_mutable::BootcToMutable, fstab::Fstab, grub2::GRUB2, nvidia_akmods::NvidiaAkmods,
        prepare_fedora::PrepareFedora, script::Script, selinux::SELinux,
    };
    vec![
        Fstab.into(),
        BootcToMutable.into(),
        GRUB2.into(),
        NvidiaAkmods.into(),
        PrepareFedora.into(),
        SELinux.into(),
        Script.into(),
    ]
}

impl InstallationType {
    #[must_use]
    pub fn cfgdir(&self, is_bootc: bool) -> std::path::PathBuf {
        match self {
            Self::ChromebookInstall => repart_dir().join("chromebookinstall"),
            Self::WholeDisk if is_bootc => repart_dir().join("bootcwholedisk"),
            Self::WholeDisk => repart_dir().join("wholedisk"),
            Self::DualBoot(_) => repart_dir().join("dualboot"),
            Self::Custom => unreachable!("custom installs do not use repart templates"),
        }
    }
}

/// # Errors
/// - cannot read config file
#[allow(clippy::module_name_repetitions)]
#[tracing::instrument]
pub fn get_cfg() -> Result<ReadymadeConfig> {
    let path = std::env::var("READYMADE_CONFIG");
    match &path {
        Err(std::env::VarError::NotUnicode(s)) => {
            tracing::error!(
                ?s,
                "Cannot parse READYMADE_CONFIG due to invalid unicode; falling back to {DEFAULT_CFG_PATH}"
            );
        }
        Ok(p) => tracing::debug!("Using READYMADE_CONFIG={p}"),
        Err(std::env::VarError::NotPresent) => tracing::trace!("Using {DEFAULT_CFG_PATH}"),
    }
    let path = path.as_deref().unwrap_or(DEFAULT_CFG_PATH);
    let toml = std::fs::read_to_string(path)
        .map_err(|e| eyre!("Cannot read config file at {path:?}").wrap_err(e))?;
    let cfg = ReadymadeConfig::from_toml_str(&toml)?;
    cfg.check()?;
    Ok(cfg)
}

impl From<&ReadymadeConfig> for ApplicationState {
    fn from(value: &crate::cfg::ReadymadeConfig) -> Self {
        let nvidia_detected = libreadymade::backend::util::sys::has_nvidia_display();
        tracing::debug!(nvidia_detected, "Probed PCI display devices");
        Self::new(value, nvidia_detected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cfg() {
        assert_eq!(
            ReadymadeConfig::from_toml_str(
                r#"
                [distro]
                name = "Ultramarine Linux"

                [install]
                allowed_installtypes = ["chromebookinstall"]
                copy_mode = "bootc"

                [[bento]]
                title = "bento1"
                desc = "bento1-desc"
                link = "https://wiki.ultramarine-linux.org/en/welcome/"
                icon = "explore-symbolic"

                [[bento]]
                title = "bento2"
                desc = "bento2-desc"
                link = "https://wiki.ultramarine-linux.org/en/community/community/"
                icon = "chat-symbolic"

                [[bento]]
                title = "bento3"
                desc = "bento3-desc"
                link = "https://wiki.ultramarine-linux.org/en/contributing/contributorguide/"
                icon = "applications-development-symbolic"

                [[postinstall]]
                module = "GRUB2"

                [[postinstall]]
                module = "CleanupBoot"

                [[postinstall]]
                module = "ReinstallKernel"

                [[postinstall]]
                module = "Dracut"

                [[postinstall]]
                module = "PrepareFedora"

                [[postinstall]]
                module = "SELinux"
                "#
            )
            .unwrap(),
            ReadymadeConfig {
                no_langpage: false,
                distro: Distro {
                    name: "Ultramarine Linux".into(),
                    icon: "fedora-logo-icon".into(),
                    bios_support: false,
                },
                install: Install {
                    allowed_installtypes: vec![InstallationType::ChromebookInstall],
                    copy_mode: CopyMode::Bootc,
                    bootc_imgref: None,
                    bootc_target_imgref: None,
                    bootc_enforce_sigpolicy: false,
                    bootc_kargs: None,
                    bootc_args: None,
                    images: vec![],
                    nvidia_secureboot_docs: _default_nvidia_secureboot_docs(),
                },
                postinstall: vec![
                    libreadymade::backend::postinstall::grub2::GRUB2.into(),
                    libreadymade::backend::postinstall::cleanup_boot::CleanupBoot.into(),
                    libreadymade::backend::postinstall::reinstall_kernel::ReinstallKernel.into(),
                    libreadymade::backend::postinstall::dracut::Dracut.into(),
                    libreadymade::backend::postinstall::prepare_fedora::PrepareFedora.into(),
                    libreadymade::backend::postinstall::selinux::SELinux.into(),
                ],
                mutable_postinstall: None,
                bentos: [
                    Bento {
                        title: "bento1".into(),
                        desc: "bento1-desc".into(),
                        link: "https://wiki.ultramarine-linux.org/en/welcome/".to_owned(),
                        icon: "explore-symbolic".into()
                    },
                    Bento {
                        title: "bento2".into(),
                        desc: "bento2-desc".into(),
                        link: "https://wiki.ultramarine-linux.org/en/community/community/"
                            .to_owned(),
                        icon: "chat-symbolic".into()
                    },
                    Bento {
                        title: "bento3".into(),
                        desc: "bento3-desc".into(),
                        link:
                            "https://wiki.ultramarine-linux.org/en/contributing/contributorguide/"
                                .to_owned(),
                        icon: "applications-development-symbolic".into(),
                    },
                ]
            },
        );
    }

    const BENTOS: &str = r#"
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

    fn parse(install: &str) -> ReadymadeConfig {
        ReadymadeConfig::from_toml_str(&format!(
            "postinstall = []\n[distro]\nname = \"Test\"\n[install]\nallowed_installtypes = [\"wholedisk\"]\n{install}\n{BENTOS}"
        ))
        .unwrap()
    }

    #[test]
    fn legacy_single_image_is_bootc_only() {
        let cfg = parse(
            r#"
            copy_mode = "bootc"
            bootc_imgref = "containers-storage:example/os:44"
            bootc_target_imgref = "docker://example/os:44"
            "#,
        );
        cfg.check().unwrap();
        assert_eq!(
            cfg.images(),
            vec![Image {
                imgref: "containers-storage:example/os:44".into(),
                target_imgref: Some("docker://example/os:44".into()),
                nvidia: false,
                modes: vec![ImageMode::Bootc],
            }]
        );
        assert_eq!(cfg.image_modes(false), vec![ImageMode::Bootc]);
        assert!(cfg.image_modes(true).is_empty());
        assert!(!cfg.has_nvidia_image());
    }

    #[test]
    fn repart_config_has_no_images() {
        let cfg = parse("");
        cfg.check().unwrap();
        assert!(cfg.images().is_empty());
        assert!(cfg.select_image(false, ImageMode::Bootc).is_none());
    }

    #[test]
    fn selects_image_by_nvidia_and_mode() {
        let cfg = parse(
            r#"
            copy_mode = "bootc"

            [[install.images]]
            imgref = "containers-storage:example/server:44"
            target_imgref = "docker://example/server:44"

            [[install.images]]
            imgref = "containers-storage:example/server-nvidia:44"
            nvidia = true
            modes = ["mutable"]
            "#,
        );
        cfg.check().unwrap();
        assert!(cfg.has_nvidia_image());
        assert_eq!(
            cfg.image_modes(false),
            vec![ImageMode::Bootc, ImageMode::Mutable]
        );
        assert_eq!(cfg.image_modes(true), vec![ImageMode::Mutable]);
        assert_eq!(
            cfg.select_image(false, ImageMode::Mutable).unwrap().imgref,
            "containers-storage:example/server:44"
        );
        assert_eq!(
            cfg.select_image(true, ImageMode::Mutable).unwrap().imgref,
            "containers-storage:example/server-nvidia:44"
        );
        assert!(cfg.select_image(true, ImageMode::Bootc).is_none());
    }

    #[test]
    fn images_take_precedence_over_bootc_imgref() {
        let cfg = parse(
            r#"
            copy_mode = "bootc"
            bootc_imgref = "containers-storage:example/legacy:44"

            [[install.images]]
            imgref = "containers-storage:example/new:44"
            "#,
        );
        assert_eq!(cfg.images().len(), 1);
        assert_eq!(cfg.images()[0].imgref, "containers-storage:example/new:44");
    }

    #[test]
    fn rejects_images_without_bootc_copy_mode() {
        let cfg = parse(
            r#"
            [[install.images]]
            imgref = "containers-storage:example/os:44"
            "#,
        );
        assert!(cfg.check().is_err());
    }

    #[test]
    fn rejects_image_without_modes() {
        let cfg = parse(
            r#"
            copy_mode = "bootc"
            [[install.images]]
            imgref = "containers-storage:example/os:44"
            modes = []
            "#,
        );
        assert!(cfg.check().is_err());
    }

    #[test]
    fn mutable_postinstall_defaults() {
        let cfg = parse("");
        assert_eq!(cfg.mutable_postinstall(), default_mutable_postinstall());
    }
}
