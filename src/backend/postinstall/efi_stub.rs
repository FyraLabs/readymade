use color_eyre::{eyre::bail, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

use super::{Context, PostInstallModule};

const EFI_SHIM_X86_64: &str = "\\EFI\\fedora\\shimx64.efi";
const EFI_SHIM_AA64: &str = "\\EFI\\fedora\\shimaa64.efi";
const OS_NAME: &str = "Ultramarine Linux";

const fn get_shim_path() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        EFI_SHIM_X86_64
    } else {
        EFI_SHIM_AA64
    }
}

/// Get partition number from partition path
///
/// # Arguments
///
/// * `partition_path` - A string slice that holds the path to the partition
///
/// # Returns
///
/// * `Result<String>` - A string that holds the partition number
///
/// # Errors
///
/// - If the partition number cannot be extracted from the partition path
/// - The path is not a valid device path
/// - The path is a whole disk, not a partition
///
/// # Example
///
/// ```rust
///
/// let partition_path = "/dev/sda1";
/// let partition_number = get_partition_number(partition_path);
///
/// assert_eq!(partition_number.unwrap(), 1);
///
/// let partition_path = "/dev/nvme0n1p2";
/// let partition_number = get_partition_number(partition_path);
///
/// assert_eq!(partition_number.unwrap(), 2);
///
/// ```
fn partition_number(partition_path: &str) -> Result<usize> {
    if !partition_path.starts_with("/dev/") {
        bail!("Not a valid device path");
    }

    // Table of known block device prefixes
    // Simple number suffix: /dev/sdXN, /dev/vdXN
    // pY suffix: /dev/nvmeXpY, /dev/mmcblkXpY, /dev/loopXpY

    if partition_path.starts_with("/dev/sd") || partition_path.starts_with("/dev/vd") {
        let partition_number = partition_path
            .chars()
            .skip_while(|c| c.is_alphabetic())
            .collect::<String>();

        return Ok(partition_number.parse::<usize>()?);
    }

    if partition_path.starts_with("/dev/nvme")
        || partition_path.starts_with("/dev/mmcblk")
        || partition_path.starts_with("/dev/loop")
    {
        let partition_number = partition_path
            .chars()
            .skip_while(|c| c.is_alphabetic())
            .collect::<String>();

        if let Some(partition_number) = partition_number.split('p').collect::<Vec<&str>>().get(1) {
            return Ok(partition_number.parse::<usize>()?);
        }

        bail!("Could not extract partition number");
    }

    bail!("Could not extract partition number");
}

/// Get the whole disk from a partition path. i.e. /dev/sda1 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1
fn get_whole_disk(partition_path: &str) -> String {
    if partition_path.starts_with("/dev/sd") || partition_path.starts_with("/dev/vd") {
        return partition_path
            .chars()
            .take_while(|c| c.is_alphabetic() || c.is_numeric())
            .collect();
    }

    if partition_path.starts_with("/dev/nvme") || partition_path.starts_with("/dev/mmcblk") {
        return partition_path
            .chars()
            .take_while(|c| c.is_alphabetic() || c.is_numeric())
            .collect();
    }

    partition_path.to_owned()
}

/// Generate an EFI stub for the bootloader
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EfiStub;

impl PostInstallModule for EfiStub {
    fn run(&self, context: &Context) -> Result<()> {
        // two guard clauses for checking EFI and
        // existence of an ESP partition
        if !context.uefi {
            return Ok(());
        }

        // if context.esp_partition.is_none() {
        //     bail!("No ESP partition found, cannot generate EFI stub");
        // }

        let Some(esp_partition) = context.esp_partition.as_ref() else {
            bail!("No ESP partition found, cannot generate EFI stub")
        };
        // get the partition number
        let partition_number = partition_number(esp_partition)?;

        let status = Command::new("efibootmgr")
            .arg("--create")
            .arg("--disk")
            .arg(get_whole_disk(esp_partition))
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

        // we will be using efibootmgr

        Ok(())
    }
}
