mod osprobe;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use lsblk::Populate;
use osprobe::OSProbe;

use crate::pages::destination::DiskInit;

/// Try and scan the system for disks and their installed OS
pub fn detect_os() -> Vec<DiskInit> {
    let disks = lsblk::BlockDevice::list().unwrap();
    let live_device = find_live_device();

    tracing::debug!(?disks, "Found disks");

    let osprobe: HashMap<_, _> = OSProbe::scan()
        .map(|probe| (probe.into_iter().map(|os| (os.part, os.os_name_pretty))).collect())
        .unwrap_or_default();

    (disks.into_iter())
        .filter(|disk| is_valid_disk(live_device.as_deref(), disk))
        .map(|disk| make_disk_init(&osprobe, disk))
        .collect()
}

fn make_disk_init(osprobe: &HashMap<PathBuf, String>, mut disk: lsblk::BlockDevice) -> DiskInit {
    let model =
        (disk.sysfs()).and_then(|p| std::fs::read_to_string(p.join("device").join("model")));
    let ret = DiskInit {
        disk_name: (model.map(|s| s.trim().to_owned()).ok())
            .or_else(|| disk.label.take().or_else(|| disk.id.take()))
            .map_or_else(|| disk.name.clone(), |s| format!("{s} ({})", disk.name)),
        os_name: (osprobe.iter())
            .filter_map(|(path, osname)| path.to_str().zip(Some(osname)))
            .find_map(|(path, osname)| {
                path.starts_with(disk.fullname.to_str().unwrap())
                    .then_some(osname)
            })
            .map_or(crate::t!("unknown-os"), ToOwned::to_owned),
        size: bytesize::ByteSize::kib(disk.capacity().unwrap().unwrap() >> 1),
        devpath: disk.fullname,
    };
    tracing::debug!(?ret, "Found disk");
    ret
}

fn is_valid_disk(live_device: Option<&str>, disk: &lsblk::BlockDevice) -> bool {
    disk.is_disk()
        && live_device != Some(&disk.name)
        && (cfg!(debug_assertions) || disk.is_physical())
        && !matches!(disk.capacity(), Err(_) | Ok(Some(0))) // #71
}

fn find_live_device() -> Option<String> {
    // Surprisingly, getting the physical device for the booted system is non trivial on Linux
    // For live systems, we can look for the the device associated with the live mountpoint
    // This way, we can avoid showing the live system as a valid target
    lsblk::Mount::list()
        .unwrap()
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
