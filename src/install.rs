//! Albius Recipe generation code for Readymade
//! This module contains the code to generate a `albius::Recipe` object that can be fed into the `albius` binary.
//! So we can actually install something with Readymade.

use crate::disks::partition;
use crate::util;
use crate::util::array_str_to_values;
use crate::InstallationState;
use color_eyre::Result;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}
