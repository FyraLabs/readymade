use bytesize::ByteSize;
use color_eyre::eyre::eyre;
use std::fmt::Write;
// use lsblk::mountpoints;
use std::path::PathBuf;
use sys_mount::MountFlags;
use tiffin::{Container, MountTarget};

use crate::backend::repartcfg::{FileSystem, RepartConfig};

/// Gets the systemd version
pub fn systemd_version() -> color_eyre::Result<usize> {
    let output = std::process::Command::new("systemctl")
        .arg("--version")
        .output()?;
    let version_str = std::str::from_utf8(&output.stdout)?;
    let version = version_str
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| eyre!("Could not parse systemd version"))?
        .split('.')
        .next()
        .ok_or_else(|| eyre!("Could not parse systemd version"))?
        .parse()?;
    Ok(version)
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
// type should be just an array of partitions
#[serde(transparent)]
pub struct RepartOutput {
    #[serde(flatten)]
    pub partitions: Vec<RepartPartition>,
}

impl RepartOutput {
    /// Generate a `BTreeMap` of mountpoint -> node name for generating /etc/fstab
    /// from DDI partition types
    pub fn mountpoints(&self) -> impl Iterator<Item = (&'static str, String)> + '_ {
        self.partitions
            .iter()
            .filter_map(|part| part.ddi_mountpoint().map(|mp| (mp, part.node.clone())))
    }

    pub fn find_by_node(&self, node: &str) -> Option<&RepartPartition> {
        self.partitions.iter().find(|part| part.node == node)
    }

    /// Generate a /etc/fstab file from the DDI partition types
    ///
    /// This function may be deprecated when systemd 256 hits f40, or when
    /// we rebase to f41
    pub fn generate_fstab(&self) -> String {
        let mut fstab = String::new();

        for part in &self.partitions {
            if let Some(_mntpnt) = part.mount_point() {
                if let Ok(fstab_entry) = part.fstab_entry() {
                    writeln!(&mut fstab, "{fstab_entry}").unwrap();
                }
            }
        }

        fstab
    }

    /// Create `tiffin::Container` from the repartitioning output with the mountpoints
    /// from the DDI partition types
    pub fn to_container(&self) -> color_eyre::Result<Container> {
        let temp_dir = tempfile::tempdir()?.into_path();

        let mut container = Container::new(temp_dir);

        for (mntpoint, node) in self.mountpoints() {
            // strip
            // mntpoint.trim_start_matches('/')
            let mnt_target = MountTarget {
                target: PathBuf::from(mntpoint),
                flags: MountFlags::empty(),
                data: None,
                fstype: None,
            };

            container.add_mount(mnt_target, PathBuf::from(node));
        }

        Ok(container)
    }
}

impl std::str::FromStr for RepartOutput {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RepartPartition {
    // "type"
    #[serde(rename = "type")]
    part_type: String,
    label: String,
    uuid: uuid::Uuid,
    partno: i32,
    file: PathBuf,
    node: String,
    offset: usize,
    old_size: ByteSize,
    raw_size: ByteSize,
    old_padding: usize,
    raw_padding: usize,
    activity: String,
}
impl RepartPartition {
    /// Returns a Discoverable Disk Image (DDI) mountpoint if defined
    /// by checking the partition type
    pub fn ddi_mountpoint(&self) -> Option<&'static str> {
        match self.part_type.as_str() {
            "xbootldr" => Some("/boot"),
            "esp" => Some("/boot/efi"),
            x if x.starts_with("root-") => Some("/"),
            _ => None,
        }
    }

    /// Generate an FS Table entry for the partition,
    /// Returns a line for /etc/fstab
    pub fn fstab_entry(&self) -> Result<String, Box<dyn std::error::Error>> {
        const FALLBACK_FS: &str = "auto";
        const FALLBACK_OPTS: &str = "defaults";
        const FALLBACK_DUMP: i32 = 0;
        const FALLBACK_PASS: i32 = 2;

        // Now, let's read the config file this thing is generated from...

        // This will try to read the repart config file this thing is from,
        // which means that this will fail if the file is not there
        // but in most cases, it should be there if you're running this anyway since
        // the output is generated from the config file on the same run

        let file_config = std::fs::read_to_string(&self.file)?;

        // Now, let's parse the config file

        let config: RepartConfig = serde_ini::from_str(&file_config)?;

        let (mntpoint, mntpoint_opts) = config.partition.mount_point_as_tuple().unwrap_or_else(
            // guess from DDI?
            || {
                self.ddi_mountpoint().map_or_else(
                    || (String::new(), None),
                    |mntpoint| (mntpoint.to_owned(), None),
                )
            },
        );

        // get fs type
        let fs_fmt = &config.partition.format;

        // serialize fs into string, if it's not there, use the fallback
        // use serde::Serialize;
        let fs_fmt_str = serde_json::to_string(fs_fmt)
            .unwrap_or_else(|_| FALLBACK_FS.to_owned())
            .replace('"', "");

        let mut mount_opts = String::new();

        // In case we have btrfs

        if let Some(FileSystem::Btrfs) = fs_fmt {
            if let Some(default_subvol) = &config.partition.default_subvolume {
                write!(&mut mount_opts, "subvol={default_subvol},")?;
            }
        }

        if let Some(opts) = mntpoint_opts {
            write!(&mut mount_opts, "{opts}")?;
        }

        // if we still have no mount options, use the fallback

        if mount_opts.is_empty() {
            mount_opts.push_str(FALLBACK_OPTS);
        }

        // let's get the UUID

        let uuid = self.uuid.to_string();

        // let's get the dump and pass values

        let dump = FALLBACK_DUMP; //todo: is there a config option for this?

        let pass = {
            // We will be checking from filesystem type

            // or the root device it should be 1. For other partitions it should be 2, or 0 to disable checking.

            // If the root file system is btrfs or XFS, the fsck order should be set to 0 instead of 1.

            if let Some(FileSystem::Btrfs) = fs_fmt {
                0
            } else if let Some(FileSystem::Xfs) = fs_fmt {
                0
            } else if mntpoint == "/" {
                1
            } else {
                FALLBACK_PASS
            }
        };

        // now let's write the fstab entry
        Ok(format!(
            "PARTUUID={uuid}\t{mntpoint}\t{fs_fmt_str}\t{mount_opts}\t{dump}\t{pass}"
        ))
    }

    pub fn mount_point(&self) -> Option<String> {
        // Read the config file or guess from DDI or return None

        let file_config = std::fs::read_to_string(&self.file).ok()?;
        let config: RepartConfig = serde_ini::from_str(&file_config).ok()?;

        config.partition.mount_point.clone().map(|_| {
            let (m, _) = config.partition.mount_point_as_tuple().unwrap_or_else(|| {
                self.ddi_mountpoint().map_or_else(
                    || (String::new(), None),
                    |mntpoint| (mntpoint.to_owned(), None),
                )
            });
            m
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const OUTPUT_EXAMPLE: &str = include_str!("repart-out.json");

    fn deserialize() -> RepartOutput {
        let val = serde_json::from_str(OUTPUT_EXAMPLE).unwrap();
        // println!("{:#?}", val);
        let output: RepartOutput = val;
        println!("{output:#?}");
        output
    }

    #[test]
    fn test_deserialize() {
        let output = deserialize();
        assert_eq!(output.partitions.len(), 4);
    }

    #[test]
    fn test_mountpoints() {
        let output = deserialize();
        let mountpoints = output
            .mountpoints()
            .collect::<std::collections::BTreeMap<_, _>>();
        println!("{mountpoints:#?}");
        assert_eq!(mountpoints.len(), 3);
        assert_eq!(mountpoints.get("/boot"), Some(&"/dev/sda3".to_owned()));
        assert_eq!(mountpoints.get("/boot/efi"), Some(&"/dev/sda1".to_owned()));
        assert_eq!(mountpoints.get("/"), Some(&"/dev/sda4".to_owned()));
    }

    #[test]
    fn test_fstab() {
        let output = deserialize();
        let mountpoints = output.generate_fstab();

        println!("{mountpoints}");
    }

    #[test]
    fn get_systemd_version() -> color_eyre::Result<()> {
        let version = systemd_version()?;
        println!("{version}");
        Ok(())
    }
}
