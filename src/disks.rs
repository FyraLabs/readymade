//! Disks scanning module for `readymade`

// todo: figure out a way to find installed OS on disks and its partitions

mod osprobe;

use osprobe::OSProbe;
use std::collections::HashMap;

use crate::pages::destination::DiskInit;

const OSNAME_PLACEHOLDER: &str = "Unknown OS";

/// Try and scan the system for disks and their installed OS
// Honestly, this is a mess and I have no idea how to get os_detect to work.
// I cannot test this function because my system only has one OS installed.
// to someone who multiboots, please fix this function for me. Thanks. - @korewaChino
// NOTE: Below system detection might not even work at all, I have no idea since above note.
pub fn detect_os() -> Vec<DiskInit> {
    let disks = lsblk::BlockDevice::list().unwrap();

    println!("{:?}", disks);

    let osprobe: HashMap<_, _> = OSProbe::scan()
        .map(|probe| (probe.into_iter().map(|os| (os.part, os.os_name_pretty))).collect())
        .unwrap_or_default();

    disks
        .into_iter()
        .filter(lsblk::BlockDevice::is_disk)
        .map(|mut disk| {
            let model = disk
                .sysfs()
                .and_then(|p| std::fs::read_to_string(p.join("device").join("model")));
            let ret = DiskInit {
                disk_name: model
                    .map(|s| s.trim().to_owned())
                    .ok()
                    .or(disk.label.take().or(disk.id.take()))
                    .map_or_else(|| disk.name.to_owned(), |s| format!("{s} ({})", disk.name)),
                os_name: osprobe
                    .iter()
                    .filter_map(|(path, osname)| path.to_str().zip(Some(osname)))
                    .find_map(|(path, osname)| path.starts_with(&disk.name).then_some(osname))
                    .map_or(OSNAME_PLACEHOLDER.to_owned(), |osname| osname.to_owned()),
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
    assert!(devices.is_ok());
}
