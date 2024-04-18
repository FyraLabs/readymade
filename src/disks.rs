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
        Self { disk_name, os_name }
    }
    pub fn into_init(self) -> DiskInit {
        DiskInit {
            disk_name: self.disk_name,
            os_name: self.os_name,
        }
    }
}

pub fn parse_os(os: os_detect::OS) -> String {
    match os {
        os_detect::OS::Windows(title) => format!("Windows ({})", title),
        os_detect::OS::MacOs(title) => format!("macOS ({})", title),
        os_detect::OS::Linux { info, .. } => format!("{}", info.pretty_name),
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

fn find_efi_parts() -> Vec<LsblkOutput> {
    let efiparts =
        cmd_lib::run_fun!(lsblk -o path,uuid,PARTTYPE,PARTTYPENAME | grep -E "EFI System$" )
            .unwrap();

    println!("{:#?}", efiparts);

    let mut out = vec![];

    // split by newline

    for line in efiparts.split("\n") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let path = parts[0];
        let uuid = parts[1];
        let parttype = parts[2];
        let parttypename = parts[3];

        let lsblk_output = LsblkOutput {
            path: path.to_string(),
            uuid: uuid.to_string(),
            parttype: parttype.to_string(),
            parttypename: parttypename.to_string(),
        };

        out.push(lsblk_output.clone());
    }

    out
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

    const PLACEHOLDER: &str = "Unknown";
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
            diskname = devpath.clone();
        }
        let partitions = cmd_lib::run_fun!(lsblk -o path,uuid,PARTTYPE,PARTTYPENAME).unwrap();
        let mut os_name = PLACEHOLDER.to_string();

        if !partitions.is_empty() {
            // list all partitions on disk

            // println!("{:#?}", partitions);

            // split by newline
            // !? what the fuck am i doing

            for line in partitions.split("\n") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 {
                    continue;
                }
                let path = parts[0];
                let uuid = parts[1];
                let parttype = parts[2];
                let parttypename = parts[3];

                let lsblk_output = LsblkOutput {
                    path: path.to_string(),
                    uuid: uuid.to_string(),
                    parttype: parttype.to_string(),
                    parttypename: parttypename.to_string(),
                };

                // println!("{:#?}", lsblk_output);


                // Don't let the thing replace all disks
                if !lsblk_output.match_device(&devpath) {
                    continue;
                }

                // run os_detect on each partition

                let a = os_detect::detect_os_from_device(Path::new(&lsblk_output.path), "auto");

                println!("tried os: {:#?}", a);

                if let Some(os) = a {
                    let name = parse_os(os);

                    os_name = name.clone();
                    break;
                }

                // let b = os_detect::detect_os_from_path(Path::new("/"));
                // println!("os: {:#?}", b);
            }
        };

        let disk = Disk::new(diskname, os_name.to_string());
        vec_diskinit.push(disk.into_init());
    }

    vec_diskinit

    // search for disks on the system
}

#[test]
fn list_disks() {
    detect_os();
}
