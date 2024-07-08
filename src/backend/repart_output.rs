use bytesize::ByteSize;
use color_eyre::eyre::eyre;
use std::fmt::Write;
// use lsblk::mountpoints;
use std::path::{Path, PathBuf};
use sys_mount::MountFlags;
use tiffin::{Container, MountTarget};

/// Gets the systemd version
pub fn systemd_version() -> color_eyre::Result<usize> {
    let output = std::process::Command::new("systemctl")
        .arg("--version")
        .output()?;
    let version = std::str::from_utf8(&output.stdout)?;
    let version = version
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| eyre!("Could not parse systemd version"))?;
    let version = version
        .split('.')
        .next()
        .ok_or_else(|| eyre!("Could not parse systemd version"))?;
    let version = version.parse()?;
    Ok(version)
}

// use super::repartcfg::PartTypeIdent;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
// type should be just an array of partitions
#[serde(transparent)]
pub struct RepartOutput {
    #[serde(flatten)]
    pub partitions: Vec<RepartPartition>,
}

impl RepartOutput {
    pub fn get_partition(&self, partno: i32) -> Option<&RepartPartition> {
        self.partitions.iter().find(|part| part.partno == partno)
    }

    /// Generate a `BTreeMap` of mountpoint -> node name for generating /etc/fstab
    /// from DDI partition types
    pub fn mountpoints(&self) -> std::collections::BTreeMap<&'static str, String> {
        self.partitions
            .iter()
            .filter_map(|part| part.ddi_mountpoint().map(|mp| (mp, part.node.clone())))
            .collect()
    }

    pub fn find_by_node(&self, node: &str) -> Option<&RepartPartition> {
        self.partitions.iter().find(|part| part.node == node)
    }

    /// Generate a /etc/fstab file from the DDI partition types
    ///
    /// This function may be deprecated when systemd 256 hits f40, or when
    /// we rebase to f41
    pub fn generate_fstab(&self) -> String {
        let mountpoints = self.mountpoints();
        let mut fstab = String::new();

        for (mntpoint, node) in mountpoints {
            let part = self.find_by_node(&node).unwrap();
            let fs_type = part.part_type.as_str();
            let uuid = part.uuid.to_string();
            let options = "defaults";
            let dump = 0;
            let pass = 2;

            write!(
                fstab,
                "UUID={uuid}\t{mntpoint}\t{fs_type}\t{options}\t{dump}\t{pass}\n"
            )
            .unwrap();
        }

        fstab
    }

    /// Create `tiffin::Container` from the repartitioning output with the mountpoints
    /// from the DDI partition types
    pub fn to_container(&self) -> color_eyre::Result<Container> {
        let mountpoints = self.mountpoints();

        let temp_dir = tempfile::tempdir()?.into_path();

        let mut container = Container::new(temp_dir);

        for (mntpoint, node) in &mountpoints {
            // strip
            // mntpoint.trim_start_matches('/')
            let mnt_target = MountTarget {
                target: PathBuf::from(mntpoint),
                flags: MountFlags::empty(),
                data: None,
                fstype: None,
            };

            container.add_mount(mnt_target, Path::new(node));
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
        let mountpoints = output.mountpoints();
        println!("{mountpoints:#?}");
        assert_eq!(mountpoints.len(), 3);
        assert_eq!(mountpoints.get("/boot"), Some(&"/dev/sda3".to_string()));
        assert_eq!(mountpoints.get("/boot/efi"), Some(&"/dev/sda1".to_string()));
        assert_eq!(mountpoints.get("/"), Some(&"/dev/sda4".to_string()));
    }

    #[test]
    fn get_systemd_version() -> color_eyre::Result<()> {
        let version = systemd_version()?;
        println!("{}", version);
        Ok(())
    }
}
