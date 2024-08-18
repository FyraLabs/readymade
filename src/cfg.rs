#![allow(clippy::str_to_string)]
use color_eyre::eyre::eyre;
use color_eyre::Result;
use serde::Deserialize;
use serde_valid::toml::FromTomlStr;
use serde_valid::Validate;

const DEFAULT_CFG_PATH: &str = "/etc/readymade.toml";

#[derive(Deserialize, Validate, Default, Debug, Clone, PartialEq, Eq)]
pub struct Install {
    #[validate(min_items = 1)]
    pub allowed_installtypes: Vec<crate::install::InstallationType>,
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
    pub install: Install,
    pub distro: Distro,
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
                [install]
                allowed_installtypes = ["chromebookinstall"]

                [distro]
                name = "Ultramarine Linux"
                "#
            )
            .unwrap(),
            ReadymadeConfig {
                install: Install {
                    allowed_installtypes: vec![crate::install::InstallationType::ChromebookInstall],
                },
                distro: Distro {
                    name: "Ultramarine Linux".into(),
                    icon: "fedora-logo-icon".into(),
                },
            },
        );
    }
}
