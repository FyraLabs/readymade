//! Disks scanning module for `readymade`

// todo: figure out a way to find installed OS on disks and its partitions

mod osprobe;

use osprobe::OSProbe;
use std::collections::HashMap;

use crate::pages::destination::DiskInit;

const OSNAME_PLACEHOLDER: &str = "Unknown";

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
        .map(|disk| {
            let ret = DiskInit {
                disk_name: format!(
                    "{} ({})",
                    disk.label
                        .as_deref()
                        .or(disk.id.as_deref())
                        .map_or("".into(), |s| format!("{s} ")),
                    disk.name,
                )
                .trim()
                .to_string(),
                os_name: osprobe
                    .iter()
                    .filter_map(|(path, osname)| path.to_str().zip(Some(osname)))
                    .find(|(path, _)| path.starts_with(&disk.name))
                    .map(|(_, osname)| osname.to_string())
                    .unwrap_or(OSNAME_PLACEHOLDER.to_string()),
                devpath: disk.fullname.clone(),
                size: bytesize::ByteSize::kib(disk.capacity().unwrap().unwrap() >> 1),
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
