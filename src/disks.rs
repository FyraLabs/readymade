//! Disks scanning module for `readymade`

// todo: figure out a way to find installed OS on disks and its partitions

use std::path::Path;

use crate::pages::destination::DiskInit;

struct Disk {
    disk_name: String,
    os_name: String,
}

impl Disk {
    pub fn new(disk_name: String, os_name: String) -> Self {
        Self {
            disk_name,
            os_name,
        }
    }
    pub fn into_init(self) -> DiskInit {
        DiskInit {
            disk_name: self.disk_name,
            os_name: self.os_name,
        }
    }
}

/// List all disks on the system
pub fn detect_os() -> Vec<DiskInit> {
    let disks_data = rs_drivelist::drive_list().unwrap();

    let mut vec_diskinit = Vec::new();

    const PLACEHOLDER : &str = "Unknown";
    println!("{:#?}", disks_data);

    for d in disks_data {
        let devpath = d.device;
        println!("path: {:#?}", devpath);
        let mut diskname = d.description;

        // filter devpaths to only include real disks
        // excluding zram and more

        if !Path::new(&devpath).exists() {
            continue;
        }

        if devpath.contains("zram") {
            continue;
        }

        if devpath.contains("loop") {
            continue;
        }

        if diskname.is_empty() {
            diskname = devpath;
        }

        let disk = Disk::new(diskname, PLACEHOLDER.to_string());
        vec_diskinit.push(disk.into_init());
    }


    vec_diskinit

    // search for disks on the system
}

#[test]
fn list_disks() {
    detect_os();
}
