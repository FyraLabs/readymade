//! Disks scanning module for `readymade`

// todo: figure out a way to find installed OS on disks and its partitions

pub mod init;
mod osprobe;

use osprobe::OSProbe;
use std::path::PathBuf;

use crate::pages::destination::DiskInit;

const OSNAME_PLACEHOLDER: &str = "Unknown";

/// Try and scan the system for disks and their installed OS
// Honestly, this is a mess and I have no idea how to get os_detect to work.
// I cannot test this function because my system only has one OS installed.
// to someone who multiboots, please fix this function for me. Thanks. - @korewaChino
// NOTE: Below system detection might not even work at all, I have no idea since above note.
pub fn detect_os() -> Vec<DiskInit> {
    let disks_data = rs_drivelist::drive_list().unwrap();

    // let efiparts = find_efi_parts();

    let mut osprobe = OSProbe::scan()
        .map(|probe| (probe.into_iter().map(|os| (os.part, os.os_name_pretty))).collect())
        .unwrap_or_default();

    tracing::debug!(?osprobe, "OS Probe");
    tracing::debug!(?disks_data, "Disks Data");

    (disks_data.into_iter().filter_map(_drive_list_filter))
        .map(_to_diskinit(&mut osprobe))
        .collect()
}

fn _drive_list_filter(d: rs_drivelist::device::DeviceDescriptor) -> Option<(PathBuf, String)> {
    let devpath = PathBuf::from(&d.device);
    if devpath.exists() && devpath.file_name().expect("Device is not file") != "zram" {
        Some((devpath, d.description))
    } else {
        None
    }
}

fn _to_diskinit(
    osprobe: &mut std::collections::HashMap<PathBuf, String>,
) -> impl FnMut((PathBuf, String)) -> DiskInit + '_ {
    |(devpath, desc)| {
        tracing::debug!(?devpath, "Device Path");
        let disk_name = if desc.is_empty() {
            devpath.display().to_string()
        } else {
            desc
        };

        let os_name = (osprobe.get_mut(&devpath).map(std::mem::take))
            .unwrap_or(OSNAME_PLACEHOLDER.to_string());

        DiskInit { disk_name, os_name }
    }
}

#[test]
fn list_disks() {
    detect_os();
}
