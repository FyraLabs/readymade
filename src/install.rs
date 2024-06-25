//! Albius Recipe generation code for Readymade
//! This module contains the code to generate a `albius::Recipe` object that can be fed into the `albius` binary.
//! So we can actually install something with Readymade.

#[derive(Debug, Clone)]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}
