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
#[tracing::instrument]
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
            .filter(|c| c.is_numeric())
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
            .skip_while(|c| c.is_numeric())
            .skip_while(|c| *c != 'p')
            .skip(1)
            .take_while(|c| c.is_numeric())
            .collect::<String>();

        if !partition_number.is_empty() {
            return Ok(partition_number.parse::<usize>()?);
        }

        bail!("Could not extract partition number");
    }

    bail!("Could not extract partition number");
}

/// Get the whole disk from a partition path. i.e. /dev/sda1 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1
#[tracing::instrument]
fn get_whole_disk(partition_path: &str) -> String {
    if partition_path.starts_with("/dev/sd") || partition_path.starts_with("/dev/vd") {
        if let Some(pos) = partition_path.rfind(|c: char| c.is_numeric()) {
            return partition_path[..pos].to_string();
        }
    }

    if partition_path.starts_with("/dev/nvme")
        || partition_path.starts_with("/dev/mmcblk")
        || partition_path.starts_with("/dev/loop")
    {
        // split by p
        let mut parts = partition_path.split('p');
        let partition_path = parts.next().unwrap();
        return partition_path.to_owned();
    }

    partition_path.to_owned()
}
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_partition_number() {
        let partition_path = "/dev/sda1";
        let partno = partition_number(partition_path);

        assert_eq!(partno.unwrap(), 1);

        let partition_path = "/dev/nvme0n1p2";
        let partno = partition_number(partition_path);

        assert_eq!(partno.unwrap(), 2);
    }

    #[test]
    fn test_get_whole_disk() {
        let partition_path = "/dev/sda1";
        let whole_disk = get_whole_disk(partition_path);

        assert_eq!(whole_disk, "/dev/sda");

        let partition_path = "/dev/nvme0n1p2";
        let whole_disk = get_whole_disk(partition_path);

        assert_eq!(whole_disk, "/dev/nvme0n1");
    }
}

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

        // if context.esp_partition.is_none() {
        //     bail!("No ESP partition found, cannot generate EFI stub");
        // }

        let Some(esp_partition) = context.esp_partition.as_ref() else {
            bail!("No ESP partition found, cannot generate EFI stub")
        };
        // get the partition number
        let partition_number = partition_number(esp_partition)?;
        let esp_disk = get_whole_disk(esp_partition);

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

        // we will be using efibootmgr

        Ok(())
    }
}
