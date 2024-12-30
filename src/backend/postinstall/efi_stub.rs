use color_eyre::{eyre::bail, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::{
    consts::{get_shim_path, OS_NAME},
    util::fs::{get_whole_disk, partition_number},
};

use super::{Context, PostInstallModule};

/// Generate an EFI stub for the bootloader
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EfiStub;

impl PostInstallModule for EfiStub {
    #[tracing::instrument(skip(self, context))]
    fn run(&self, context: &Context) -> Result<()> {
        // two guard clauses for checking EFI and
        // existence of an ESP partition
        if !context.uefi {
            return Ok(());
        }

        tracing::debug!(esp_part = ?context.esp_partition, uefi = ?context.uefi, "Generating EFI stub");

        let Some(esp_partition) = context.esp_partition.as_ref() else {
            bail!("No ESP partition found, cannot generate EFI stub")
        };
        // get the partition number
        let partition_number = partition_number(esp_partition)?;
        let esp_disk = get_whole_disk(esp_partition)?;

        tracing::debug!(?partition_number, "EFI partition number");
        tracing::debug!(?esp_disk, "EFI disk");

        let status = Command::new("/usr/sbin/efibootmgr")
            .arg("--create")
            .arg("--disk")
            .arg(esp_disk)
            .arg("--part")
            .arg(partition_number.to_string())
            .arg("--label")
            .arg(OS_NAME)
            .arg("--loader")
            .arg(get_shim_path())
            .status()?;

        if !status.success() {
            bail!("Failed to create EFI boot entry");
        }

        Ok(())
    }
}
