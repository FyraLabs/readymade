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
    #[tracing::instrument]
    pub fn from_entry(entry: &str) -> Self {
        let parts: Vec<&str> =
            tracing::debug_span!("OS Probe Entry", ?entry).in_scope(|| entry.split(":").collect());

        // Minimum 4 parts, Part 5, 6 and 7 are optional

        let [part, os_name_pretty, os_name, part_type, ..] = parts[..] else {
            panic!("Expected at least 4 OS Probe entries for `{entry}`, but found the following: {parts:?}");
        };

        tracing::info_span!("Serializing os-prober entry").in_scope(|| Self {
            part: part.into(),
            os_name_pretty: os_name_pretty.to_string(),
            os_name: os_name.to_string(),
            part_type: part_type.to_string(),
            part_fs: parts.get(4).map(ToString::to_string),
            part_uuid: parts.get(5).map(ToString::to_string),
            kernel_opts: parts.get(6).map(ToString::to_string),
        })
    }

    // #[tracing::instrument]
    pub fn scan() -> Option<Vec<Self>> {
        // check if root already

        const ERROR: &str = "os-prober failed to run! Are we root? Is it installed? Continuing without OS detection.";

        let scan = tracing::info_span!("Scanning for OS").in_scope(|| {
            tracing::info!("Scanning for OS with os-prober");
            (crate::util::run_as_root("os-prober").ok())
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
        });

        // let scan: Option<String> = Some("".to_string()); // test case for failure

        scan.map(|strout| {
            tracing::info!(?strout, "OS Probe Output");

            (strout.split('\n').map(|s| s.trim()))
                .filter(|l| !l.is_empty())
                .map(OSProbe::from_entry)
                .collect()
        })
        .or_else(|| {
            tracing::error!("{ERROR}");
            None
        })
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

        if devpath.contains("zram") {
            continue;
        }

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
