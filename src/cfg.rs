#![allow(clippy::str_to_string)]
use color_eyre::eyre::eyre;
use color_eyre::Result;
use serde::Deserialize;
use serde_valid::toml::FromTomlStr;
use serde_valid::Validate;

use crate::backend::install::InstallationType;
use crate::backend::postinstall::Module;

const DEFAULT_CFG_PATH: &str = "/etc/readymade.toml";

#[derive(Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CopyMode {
    #[default]
    Repart,
    Bootc,
}

#[derive(Deserialize, Validate, Default, Debug, Clone, PartialEq, Eq)]
pub struct Install {
    #[validate(min_items = 1)]
    pub allowed_installtypes: Vec<InstallationType>,
    #[serde(default)]
    pub copy_mode: CopyMode,
    pub bootc_imgref: Option<String>,
}

#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct Distro {
    pub name: String,
    #[serde(default = "_default_icon")]
    pub icon: String,
}

fn _default_icon() -> String {
    "fedora-logo-icon".into()
}

#[derive(Deserialize, Validate, Default, Debug, Clone, PartialEq, Eq)]
pub struct ReadymadeConfig {
    pub distro: Distro,
    pub install: Install,
    pub postinstall: Vec<Module>,
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
                distro: Distro {
                    name: "Ultramarine Linux".into(),
                    icon: "fedora-logo-icon".into(),
                },
                install: Install {
                    allowed_installtypes: vec![InstallationType::ChromebookInstall],
                    copy_mode: CopyMode::Repart,
                    bootc_imgref: None,
                },
                postinstall: vec![
                    crate::backend::postinstall::grub2::GRUB2.into(),
                    crate::backend::postinstall::cleanup_boot::CleanupBoot.into(),
                    crate::backend::postinstall::reinstall_kernel::ReinstallKernel.into(),
                    crate::backend::postinstall::dracut::Dracut.into(),
                    crate::backend::postinstall::prepare_fedora::PrepareFedora.into(),
                    crate::backend::postinstall::selinux::SELinux.into(),
                ]
            },
        );
    }
}
