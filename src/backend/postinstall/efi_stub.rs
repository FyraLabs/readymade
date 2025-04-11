use color_eyre::{eyre::bail, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::{
    consts::shim_path,
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

        tracing::debug!(
            disk = esp_disk,
            part = partition_number,
            label = context.distro_name,
            shim_path = shim_path(),
            "Creating EFI boot entry"
        );

        let status = Command::new("/usr/sbin/efibootmgr")
            .arg("--create")
            .arg("--disk")
            .arg(esp_disk)
            .arg("--part")
            .arg(partition_number.to_string())
            .arg("--label")
            .arg(&context.distro_name)
            .arg("--loader")
            .arg(shim_path())
            .status()?;

        if !status.success() {
            // We should be able to fail silently here, as the user may still be able to find the OS as long
            // as the firmware finds the bootloader binary

            tracing::error!("Failed to create EFI boot entry");

            tracing::warn!("EFI boot entry creation failed, You may not be able to find the installed OS in the boot menu");

            // todo: Implement a popup to warn the user, since this is not a critical error but should be noted
        }

        Ok(())
    }
}
