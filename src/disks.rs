//! Disks scanning module for `readymade`

// todo: figure out a way to find installed OS on disks and its partitions

use std::path::{Path, PathBuf};

use crate::pages::destination::DiskInit;
#[derive(Debug, Clone)]
pub struct OSProbe {
    pub part: PathBuf,
    pub os_name_pretty: String,
    pub os_name: String,
    pub part_type: String,
    pub part_fs: Option<String>,
    pub part_uuid: Option<String>,
    pub kernel_opts: Option<String>,
}

impl OSProbe {
    pub fn from_entry(entry: &str) -> Self {
        let parts: Vec<&str> = entry.split(":").collect();

        // Minimum 4 parts, Part 5, 6 and 7 are optional

        let part = PathBuf::from(parts[0]);
        let os_name_pretty = parts[1];
        let os_name = parts[2];
        let part_type = parts[3];

        let part_fs = if parts.len() > 4 {
            Some(parts[4].to_string())
        } else {
            None
        };

        let part_uuid = if parts.len() > 5 {
            Some(parts[5].to_string())
        } else {
            None
        };

        let kernel_opts = if parts.len() > 6 {
            Some(parts[6].to_string())
        } else {
            None
        };

        Self {
            part,
            os_name_pretty: os_name_pretty.to_string(),
            os_name: os_name.to_string(),
            part_type: part_type.to_string(),
            part_fs,
            part_uuid,
            kernel_opts,
        }
    }

    pub fn scan() -> Option<Vec<Self>> {

        // check if root already

        let scan = crate::util::run_as_root("os-prober").ok();

        // let scan = cmd_lib::run_fun!("os-prober").ok();

        if let Some(strout) = scan {
            let mut out = vec![];

            for line in strout.split("\n") {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let os_probe = OSProbe::from_entry(line);
                out.push(os_probe);
            }

            Some(out)
        } else {
            tracing::error!("ERROR: os-prober failed to run! Are we root? Is it installed? Continuing without OS detection.");
            return None;
        }
    }
}

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

#[derive(Debug, Clone)]
pub struct LsblkOutput {
    pub path: String,
    pub uuid: String,
    pub parttype: String,
    pub parttypename: String,
}

impl LsblkOutput {
    pub fn match_device(&self, device: &str) -> bool {
        self.path.contains(device)
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

    for d in disks_data {
        let mut os_name = PLACEHOLDER.to_string();
        let devpath = d.device;
        tracing::debug!(?devpath, "Device Path");
        let mut diskname = d.description;

        // filter devpaths to only include real disks
        // excluding zram and more

        if !Path::new(&devpath).exists() {
            continue;
        }

        // if devpath.contains("zram") {
        //     continue;
        // }

        // if devpath.contains("loop") {
        //     continue;
        // }

        if diskname.trim().is_empty() {
            diskname = devpath.clone();
        }

        if let Some(osprobe) = &osprobe {
            for os in osprobe {
                if os.part.to_str().unwrap().contains(&devpath) {
                    os_name = os.os_name_pretty.clone();
                }
            }
        }

        let disk = Disk::new(diskname, os_name);

        vec_diskinit.push(disk.into_init());
    }

    vec_diskinit

    // search for disks on the system
}

#[test]
fn list_disks() {
    detect_os();
}
