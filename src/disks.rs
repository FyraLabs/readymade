//! Disks scanning module for `readymade`

// todo: figure out a way to find installed OS on disks and its partitions

pub mod init;
mod lsblk;
mod osprobe;

use osprobe::OSProbe;
use std::path::Path;

use crate::pages::destination::DiskInit;

struct Disk {
    disk_name: String,
    os_name: String,
}

impl Disk {
    pub fn new(disk_name: String, os_name: String) -> Self {
        Self { disk_name, os_name }
    }
    pub fn into_init(self) -> DiskInit {
        DiskInit {
            disk_name: self.disk_name,
            os_name: self.os_name,
        }
    }
}

/// Try and scan the system for disks and their installed OS
// Honestly, this is a mess and I have no idea how to get os_detect to work.
// I cannot test this function because my system only has one OS installed.
// to someone who multiboots, please fix this function for me. Thanks. - @korewaChino
// NOTE: Below system detection might not even work at all, I have no idea since above note.
pub fn detect_os() -> Vec<DiskInit> {
    let disks_data = rs_drivelist::drive_list().unwrap();
    let mut vec_diskinit = Vec::new();

    // let efiparts = find_efi_parts();

    let osprobe = OSProbe::scan();

    tracing::debug!(?osprobe, "OS Probe");

    const PLACEHOLDER: &str = "Unknown";
    tracing::debug!(?disks_data, "Disks Data");

    for d in disks_data.into_iter().filter(_lsblk_filter) {
        let devpath = d.device;
        tracing::debug!(?devpath, "Device Path");
        let mut diskname = d.description;

        // filter devpaths to only include real disks
        // excluding zram and more

        if !Path::new(&devpath).exists() {
            continue;
        }

        if devpath.contains("zram") {
            continue;
        }

        // if devpath.contains("loop") {
        //     continue;
        // }

        if diskname.trim().is_empty() {
            diskname = devpath.clone();
        }

        let os_name = (osprobe.as_ref())
            .and_then(|x| {
                x.iter()
                    .find(|os| os.part.to_str().map_or(false, |s| s.contains(&devpath)))
            })
            .map_or(PLACEHOLDER.to_string(), |os| os.os_name_pretty.clone());

        let disk = Disk::new(diskname, os_name);

        vec_diskinit.push(disk.into_init());
    }

    vec_diskinit

    // search for disks on the system
}

pub fn _lsblk_filter(d: &rs_drivelist::device::DeviceDescriptor) -> bool {
    let devpath = &d.device;
    Path::new(devpath).exists() && !devpath.contains("zram")
}

#[test]
fn list_disks() {
    detect_os();
}
