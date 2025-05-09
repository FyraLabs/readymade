#![allow(clippy::str_to_string)]
use crate::prelude::*;
use serde::{Deserialize, Serialize};
use serde_valid::{toml::FromTomlStr, Validate};

use crate::backend::install::InstallationType;
use crate::backend::postinstall::Module;

#[cfg(not(debug_assertions))]
const DEFAULT_CFG_PATH: &str = "/etc/readymade.toml";

#[cfg(debug_assertions)]
const DEFAULT_CFG_PATH: &str = "templates/ultramarine.toml";

#[derive(Deserialize, Serialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CopyMode {
    #[default]
    Repart,
    Bootc,
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
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct Distro {
    pub name: String,
    #[serde(default = "_default_icon")]
    pub icon: String,
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
}

/// # Errors
/// - cannot read config file
#[allow(clippy::module_name_repetitions)]
#[tracing::instrument]
pub fn get_cfg() -> Result<ReadymadeConfig> {
    let path = std::env::var("READYMADE_CONFIG");
    match &path {
        Err(std::env::VarError::NotUnicode(s)) => {
            tracing::error!(?s, "Cannot parse READYMADE_CONFIG due to invalid unicode");
            tracing::debug!("Falling back to {DEFAULT_CFG_PATH}");
        }
        Ok(p) => tracing::debug!("Using READYMADE_CONFIG={p}"),
        Err(std::env::VarError::NotPresent) => tracing::trace!("Using {DEFAULT_CFG_PATH}"),
    }
    let path = path.as_deref().unwrap_or(DEFAULT_CFG_PATH);
    let toml = std::fs::read_to_string(path)
        .map_err(|e| eyre!("Cannot read config file at {path:?}").wrap_err(e))?;
    Ok(ReadymadeConfig::from_toml_str(&toml)?)
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
                },
                install: Install {
                    allowed_installtypes: vec![InstallationType::ChromebookInstall],
                    copy_mode: CopyMode::Bootc,
                    bootc_imgref: None,
                    bootc_target_imgref: None,
                    bootc_enforce_sigpolicy: false,
                    bootc_kargs: None,
                    bootc_args: None,
                },
                postinstall: vec![
                    crate::backend::postinstall::grub2::GRUB2.into(),
                    crate::backend::postinstall::cleanup_boot::CleanupBoot.into(),
                    crate::backend::postinstall::reinstall_kernel::ReinstallKernel.into(),
                    crate::backend::postinstall::dracut::Dracut.into(),
                    crate::backend::postinstall::prepare_fedora::PrepareFedora.into(),
                    crate::backend::postinstall::selinux::SELinux.into(),
                ],
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
}
