//! Disks scanning module for `readymade`

// todo: figure out a way to find installed OS on disks and its partitions

pub mod init;
mod osprobe;

use color_eyre::eyre::OptionExt;
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
    // d.devicePath is the device in /dev/disk/by-path, a trace of the shortest physical path to the disk
    // if it doesn't exist, the disk probably isn't a physical disk, so we ignore it
    if devpath.exists() && d.devicePath.is_some() {
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

        DiskInit { disk_name, os_name, devpath }
    }
}

pub fn partition(dev: &std::path::Path, n: u8) -> PathBuf {
    let s = dev.display();
    let str = s.to_string();
    if str.starts_with("/dev/sd") || str.starts_with("/dev/hd") || str.starts_with("/dev/vd") {
        PathBuf::from(format!("{s}{n}"))
    } else if str.starts_with("/dev/nvme") || str.starts_with("/dev/mmcblk") || str.starts_with("/dev/loop") {
        PathBuf::from(format!("{s}p{n}"))
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

#[test]
fn list_disks() {
    detect_os();
}
