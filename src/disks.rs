//! Disks scanning module for `readymade`

// todo: figure out a way to find installed OS on disks and its partitions

pub mod init;
mod osprobe;

use color_eyre::eyre::OptionExt;
use osprobe::OSProbe;
use std::{collections::HashMap, path::PathBuf};

use crate::pages::destination::DiskInit;

const OSNAME_PLACEHOLDER: &str = "Unknown";

/// Try and scan the system for disks and their installed OS
// Honestly, this is a mess and I have no idea how to get os_detect to work.
// I cannot test this function because my system only has one OS installed.
// to someone who multiboots, please fix this function for me. Thanks. - @korewaChino
// NOTE: Below system detection might not even work at all, I have no idea since above note.
pub fn detect_os() -> Vec<DiskInit> {
    let disks = lsblk::BlockDevice::list().unwrap();

    let osprobe: HashMap<_, _> = OSProbe::scan()
        .map(|probe| (probe.into_iter().map(|os| (os.part, os.os_name_pretty))).collect())
        .unwrap_or_default();

    disks
        .iter()
        .filter(|disk| disk.is_disk())
        .map(|disk| {
            let ret = DiskInit {
                // id:
                disk_name: format!(
                    "{} ({})",
                    disk.label
                        .as_deref()
                        .or(disk.id.as_deref())
                        .map_or("".into(), |s| format!("{s} ")),
                    disk.name
                )
                .trim()
                .to_string(),
                os_name: osprobe
                    .iter()
                    .filter_map(|(path, osname)| path.to_str().zip(Some(osname)))
                    .find(|(path, _)| path.starts_with(&disk.name))
                    .map(|(_, osname)| osname.to_string())
                    .unwrap_or(OSNAME_PLACEHOLDER.to_string()),
                devpath: PathBuf::from(disk.name.clone()),
            };
            tracing::debug!(?ret, "Found disk");
            ret
        })
        .collect()
}

/// Get partition path for a disk according to what kind of disk it is
///
/// # Arguments
///
/// * `dev` - Path to the disk
/// * `n` - Partition number
///
/// # Returns
///
/// * PathBuf - Path to the partition
///
#[tracing::instrument]
pub fn partition(dev: &std::path::Path, n: u8) -> PathBuf {
    tracing::trace!(?dev, ?n, "Concatenating dev path and partition number");

    // NOTE: So turns out we're actually inputting the device name here and not full path to device
    // so looking for /dev prefix would be wrong?
    // - @korewaChino

    let s = dev.display();

    let str = s.to_string();
    if str.starts_with("sd") || str.starts_with("hd") || str.starts_with("vd") {
        PathBuf::from(format!("{s}{n}"))
    } else if str.starts_with("nvme") || str.starts_with("mmcblk") || str.starts_with("loop") {
        // HACK: add /dev to path
        // todo, suggestion: Either add /dev prefix to everything that calls this function or figure out a standard method for this! If you pick the first option, remove this comment
        // and remove the /dev prefix from the return value!!

        // IMPORTANT: This is a hack and should be removed for robustness
        PathBuf::from(format!("/dev/{s}p{n}"))
    } else {
        unimplemented!() // TODO: parse other kinds of devices?
    }
}

pub fn last_part(diskpath: &std::path::Path) -> color_eyre::Result<String> {
    let sdiskpath = diskpath.display().to_string();
    let lsblk = cmd_lib::run_fun!(lsblk -o path)?;
    // assume all dev paths start with /

    (lsblk.split('\n').skip(1))
        .filter(|l| l.starts_with(&sdiskpath))
        .last() // last one is the one with max partn
        .map(|s| s.to_string())
        .ok_or_eyre(color_eyre::Report::msg("lsblk has no output"))
}

#[cfg(test)]
#[cfg(target_os = "linux")]
#[test]
fn test_lsblk_smoke() {
    let devices = lsblk::BlockDevice::list();
    assert!(devices.is_ok());
}
