//! Module for exporting Readymade's install state to a file, Useful for other tools to check
//! the initial state of the system. Not so useful when the user modifies the system after and
//! the state drifts from the initial state. (i.e the user repartitions the disk, adds a new disk,
//! spans the BTRFS volume, etc.)
//!
use std::{collections::BTreeMap, path::Path};

use color_eyre::Result;
use serde::{Deserialize, Serialize};

use super::{install::FinalInstallationState, repartcfg::RepartConfig};
/// The version of the result dump format, for backwards compat reasons
///
/// If there's any changes to the format, this should be bumped up to the next version.
///
const RESULT_DUMP_FORMAT_VERSION: &str = "0.1.0";
#[derive(Serialize, Deserialize, Debug)]
pub struct ReadymadeResult {
    pub version: &'static str,
    pub readymade_version: &'static str,
    pub is_debug_build: bool,
    pub state: FinalInstallationState,
    pub systemd_repart_data: Option<SystemdRepartData>,
}

impl ReadymadeResult {
    pub fn export_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self)?)
    }

    pub fn new(
        state: FinalInstallationState,
        systemd_repart_data: Option<SystemdRepartData>,
    ) -> Self {
        Self {
            version: RESULT_DUMP_FORMAT_VERSION,
            readymade_version: env!("CARGO_PKG_VERSION"),
            is_debug_build: cfg!(debug_assertions),
            state: prep_state_for_export(state.into()).unwrap(),
            systemd_repart_data,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemdRepartData {
    configs: BTreeMap<String, RepartConfig>,
}

impl SystemdRepartData {
    pub const fn new(configs: BTreeMap<String, RepartConfig>) -> Self {
        Self { configs }
    }

    pub fn get_configs(cfg_path: &Path) -> Result<Self> {
        let mut configs = BTreeMap::new();
        // Read path
        for entry in std::fs::read_dir(cfg_path)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let file_config = std::fs::read_to_string(&path)?;

            // Parse the config
            let config: RepartConfig = serde_systemd_unit::from_str(&file_config)?;

            // Add to the list
            configs.insert(
                path.file_name().unwrap().to_string_lossy().to_string(),
                config,
            );
        }
        Ok(Self::new(configs))
    }
}

pub fn prep_state_for_export(mut state: FinalInstallationState) -> Result<FinalInstallationState> {
    // Clear out passwords
    if let Some(super::install::EncryptState {
        ref mut encryption_key,
        ..
    }) = state.encrypts
    {
        *encryption_key = "REDACTED".to_owned();
    }
    Ok(state)
}
