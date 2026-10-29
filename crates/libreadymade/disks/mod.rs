mod osprobe;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use lsblk::Populate;
use osprobe::OSProbe;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Disk {
    pub disk_name: String,
    pub os_name: Option<String>,
    pub devpath: PathBuf,
    pub size: bytesize::ByteSize,
}

/// Try and scan the system for disks and their installed OS
///
/// Returns no disks if block devices cannot be listed.
pub fn detect_os() -> Vec<Disk> {
    let disks = match lsblk::BlockDevice::list() {
        Ok(disks) => disks,
        Err(e) => {
            tracing::error!(?e, "Cannot list block devices");
            return vec![];
        }
    };
    let live_device = find_live_device();

    tracing::debug!(?disks, "Found disks");

    // disk name (e.g. `sda`) → name of the first OS found on one of its partitions
    let mut osprobe: HashMap<String, String> = HashMap::new();
    for os in OSProbe::scan().unwrap_or_default() {
        let Some(disk) = (disks.iter())
            .find(|dev| dev.fullname == os.part)
            .and_then(|dev| dev.disk_name().ok())
        else {
            tracing::debug!(?os, "Cannot find the disk of the partition os-prober found");
            continue;
        };
        osprobe.entry(disk).or_insert(os.os_name_pretty);
    }

    (disks.into_iter())
        .filter(|disk| is_valid_disk(live_device.as_deref(), disk))
        .map(|disk| make_disk(&osprobe, disk))
        .collect()
}

fn make_disk(osprobe: &HashMap<String, String>, mut disk: lsblk::BlockDevice) -> Disk {
    let model =
        (disk.sysfs()).and_then(|p| std::fs::read_to_string(p.join("device").join("model")));
    let ret = Disk {
        disk_name: (model.map(|s| s.trim().to_owned()).ok())
            .filter(|s| !s.is_empty())
            .or_else(|| disk.label.take().or_else(|| disk.id.take()))
            .map_or_else(|| disk.name.clone(), |s| format!("{s} ({})", disk.name)),
        os_name: osprobe.get(&disk.name).cloned(),
        // capacity is in 512-byte sectors; is_valid_disk() ensures it can be read
        size: bytesize::ByteSize::kib(disk.capacity().ok().flatten().unwrap_or_default() >> 1),
        devpath: disk.fullname,
    };
    tracing::debug!(?ret, "Found disk");
    ret
}

fn is_valid_disk(live_device: Option<&str>, disk: &lsblk::BlockDevice) -> bool {
    disk.is_disk()
        && live_device != Some(&disk.name)
        && (cfg!(debug_assertions) || disk.is_physical())
        && matches!(disk.capacity(), Ok(Some(n)) if n > 0) // #71
}

fn find_live_device() -> Option<String> {
    // Surprisingly, getting the physical device for the booted system is non trivial on Linux
    // For live systems, we can look for the the device associated with the live mountpoint
    // This way, we can avoid showing the live system as a valid target
    lsblk::Mount::list()
        .inspect_err(|e| tracing::error!(?e, "Cannot list mounts"))
        .ok()?
        .find(|mount| {
            [Path::new("/run/initramfs/live"), Path::new("/")].contains(&&*mount.mountpoint)
                && mount.device.starts_with("/dev/")
        })
        .map(|mount| PathBuf::from(mount.device))
        .map(|dev| {
            let mut dev = lsblk::BlockDevice::from_abs_path_unpopulated(dev);
            _ = dev.populate_partuuid();
            dev.disk_name().unwrap_or(dev.name)
        })
}

#[cfg(test)]
#[cfg(target_os = "linux")]
#[test]
fn test_lsblk_smoke() {
    let devices = lsblk::BlockDevice::list();
    devices.unwrap();
}
