mod osprobe;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use lsblk::Populate;
use osprobe::OSProbe;

use crate::pages::destination::DiskInit;

/// Try and scan the system for disks and their installed OS
// Honestly, this is a mess and I have no idea how to get os_detect to work.
// I cannot test this function because my system only has one OS installed.
// to someone who multiboots, please fix this function for me. Thanks. - @korewaChino
// NOTE: Below system detection might not even work at all, I have no idea since above note.
pub fn detect_os() -> Vec<DiskInit> {
    let disks = lsblk::BlockDevice::list().unwrap();
    // Surprisingly, getting the physical device for the booted system is non trivial on Linux
    // For live systems, we can look for the the device associated with the live mountpoint
    // This way, we can avoid showing the live system as a valid target
    let live_device = lsblk::Mount::list()
        .unwrap()
        .find(|mount| {
            [Path::new("/run/initramfs/live"), Path::new("/")].contains(&&*mount.mountpoint)
        })
        .map(|mount| PathBuf::from(mount.device))
        .map(|dev| {
            let mut dev = lsblk::BlockDevice::from_abs_path_unpopulated(dev);
            _ = dev.populate_partuuid();
            dev.disk_name().unwrap_or(dev.name)
        });

    tracing::debug!(?disks, "Found disks");

    let osprobe: HashMap<_, _> = OSProbe::scan()
        .map(|probe| (probe.into_iter().map(|os| (os.part, os.os_name_pretty))).collect())
        .unwrap_or_default();

    disks
        .into_iter()
        .filter(|disk| {
            disk.is_disk()
                && live_device.as_ref() != Some(&disk.name)
                && (cfg!(debug_assertions) || disk.is_physical())
        })
        .map(|mut disk| {
            let model = disk
                .sysfs()
                .and_then(|p| std::fs::read_to_string(p.join("device").join("model")));
            let ret = DiskInit {
                disk_name: model
                    .map(|s| s.trim().to_owned())
                    .ok()
                    .or_else(|| disk.label.take().or_else(|| disk.id.take()))
                    .map_or_else(|| disk.name.clone(), |s| format!("{s} ({})", disk.name)),
                os_name: osprobe
                    .iter()
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
        })
        .collect()
}

#[cfg(test)]
#[cfg(target_os = "linux")]
#[test]
fn test_lsblk_smoke() {
    let devices = lsblk::BlockDevice::list();
    devices.unwrap();
}
