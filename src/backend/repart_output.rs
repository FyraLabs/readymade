use bytesize::ByteSize;
use color_eyre::eyre::Context;
use std::fmt::Write;
use std::path::PathBuf;
use sys_mount::MountFlags;
use tiffin::{Container, MountTarget};

use crate::{
    backend::repartcfg::{FileSystem, RepartConfig},
    util::sys::check_uefi,
};

/// Gets the systemd version

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
        (self.partitions.iter())
            .filter_map(|part| part.ddi_mountpoint().map(|mp| (mp, part.node.clone())))
    }

    /// Generate a /etc/fstab file from the DDI partition types
    ///
    /// This function may be deprecated when systemd 256 hits f40, or when
    /// we rebase to f41
    ///
    /// # XXX: This will read the config files in the ***CURRENT*** root context
    /// So you must have the same config files in the chroot, or exit the chroot,
    /// run this, then re-enter the chroot.
    pub fn generate_fstab(&self) -> color_eyre::Result<String> {
        let mut fstab = String::new();

        let mut partitions = vec![];
        for part in &self.partitions {
            partitions.extend(part.mount_point()?.into_iter().map(|mp| (mp, part.clone())));
        }
        // sort by mountpoint,
        // root goes first, then each subdirectory counting the slashes
        partitions.sort_by(|((a_mnt, _), _), ((b_mnt, _), _)| {
            // If either path is root (/), it should go first
            if a_mnt == "/" {
                std::cmp::Ordering::Less
            } else if b_mnt == "/" {
                std::cmp::Ordering::Greater
            } else {
                // Otherwise sort by number of slashes then alphabetically
                let a_slashes = a_mnt.chars().filter(|&c| c == '/').count();
                let b_slashes = b_mnt.chars().filter(|&c| c == '/').count();
                a_slashes.cmp(&b_slashes).then(a_mnt.cmp(b_mnt))
            }
        });

        tracing::trace!(?partitions, "Sorted partitions");

        for (mnt, part) in partitions {
            println!("Processing partition: {}", part.node);
            // if let Some(_mntpnt) = part.ddi_mountpoint() {
            tracing::trace!(?part, "Processing partition");
            let entry = part.fstab_entry(mnt)?;
            writeln!(&mut fstab, "{entry}").unwrap();
        }

        Ok(fstab)
    }

    /// Get the ESP partition if it exists
    ///
    /// This is a convenience function for getting the ESP partition, which we can then use for creating
    /// the boot stub later on
    pub fn get_esp_partition(&self) -> std::option::Option<String> {
        self.partitions
            .iter()
            .find(|part| part.part_type == "esp")
            .map(|part| part.node.clone())
    }

    pub fn get_xbootldr_partition(&self) -> std::option::Option<String> {
        self.partitions
            .iter()
            .find(|part| part.part_type == "xbootldr")
            .map(|part| part.node.clone())
    }

    /// Create `tiffin::Container` from the repartitioning output with the mountpoints
    /// from the DDI partition types
    pub fn to_container(&self, passphrase: Option<&str>) -> color_eyre::Result<Container> {
        let temp_dir = tempfile::tempdir()?.into_path();

        // let config: RepartConfig = serde_systemd_unit::from_str(&file_config)?;

        let mut container = Container::new(temp_dir.clone());

        fn is_luks(node: &str) -> bool {
            let cmd = std::process::Command::new("cryptsetup")
                .arg("isLuks")
                .arg(node)
                .output()
                .unwrap();
            cmd.status.success()
        }
        
        for part in &self.partitions {
            let rpcfg: RepartConfig =
                serde_systemd_unit::from_str(&std::fs::read_to_string(&part.file)?)?;
            let mntpoints = rpcfg.partition.mount_point_as_tuple();
            let label = part.label.clone();
            let node = PathBuf::from(&part.node);
            if rpcfg.partition.encrypt.is_on() {
                // check if isLuks
                if is_luks(&part.node) {
                    let Some(pass) = passphrase else {
                        panic!("Passphrase is empty when is_luks() is true");
                    };
                    let key_file_path = temp_dir.join("keyfile.txt");
                    let mut key_file = std::fs::File::create(&key_file_path).wrap_err("cannot create key file")?;
                    std::io::Write::write_all(&mut key_file, pass.as_bytes()).wrap_err("cannot write to key file")?;
                    drop(key_file);
                    // i guess to_container now also needs to accept an Option<String> for the passphrase
                    let cmd = std::process::Command::new("cryptsetup")
                        .arg("open")
                        .arg(&node)
                        .arg(&label)
                        .arg("--batch-mode")
                        .arg("--key-file")
                        .arg(key_file_path)
                        // todo: is there a way for cryptsetup to output the /dev/mapper path so we don't have to guess?
                        .output()?;
                    
                    if !cmd.status.success() {
                        color_eyre::eyre::bail!("cryptsetup failed: {}", String::from_utf8_lossy(&cmd.stderr));
                    }
                    
                    // TODO: mount? (/dev/mapper?)
                    // 
                    let mapper = PathBuf::from(format!("/dev/mapper/{label}"));
                    
                    for (mntpoint, mntpoint_opts) in mntpoints {
                        tracing::debug!("Mounting encrypted partition: {}", mntpoint);
                        let mnt_target = MountTarget {
                            target: PathBuf::from(mntpoint),
                            flags: MountFlags::empty(),
                            data: None,
                            fstype: None,
                        };
                        container.add_mount(mnt_target, mapper.clone());
                    }
                } else {
                    // normal mountpath
                    for (mntpoint, mntpoint_opts) in mntpoints {
                        let mnt_target = MountTarget {
                            target: PathBuf::from(mntpoint),
                            flags: MountFlags::empty(),
                            data: None,
                            fstype: None,
                        };

                        container.add_mount(mnt_target, node.clone());
                    }
                }
            }
        }

        // for (mntpoint, node) in self.mountpoints() {
        //     // strip
        //     // mntpoint.trim_start_matches('/')
        //     // let rpcfg = serde_systemd_unit::from_str(&self.partitions)?;
        //     let mnt_target = MountTarget {
        //         target: PathBuf::from(mntpoint),
        //         flags: MountFlags::empty(),
        //         data: None,
        //         fstype: None,
        //     };

        //     container.add_mount(mnt_target, PathBuf::from(node));
        // }

        if check_uefi() {
            // add efivarfs
            let mnt_target = MountTarget {
                target: PathBuf::from("/sys/firmware/efi/efivars"),
                flags: MountFlags::empty(),
                data: None,
                fstype: Some("efivarfs".to_owned()),
            };

            container.add_mount(mnt_target, PathBuf::from("/sys/firmware/efi/efivars"));
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

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone)]
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
    ///
    /// This will refer to the config file systemd-repart refers to.
    ///
    #[tracing::instrument]
    pub fn fstab_entry(
        &self,
        (mntpoint, mntpoint_opts): (String, Option<String>),
    ) -> color_eyre::Result<String> {
        const FALLBACK_FS: &str = "auto";
        const FALLBACK_OPTS: &str = "defaults";
        const FALLBACK_DUMP: i32 = 0;
        const FALLBACK_PASS: i32 = 2;

        tracing::trace!("Generating fstab entry");
        // Now, let's read the config file this thing is generated from...

        // This will try to read the repart config file this thing is from,
        // which means that this will fail if the file is not there
        // but in most cases, it should be there if you're running this anyway since
        // the output is generated from the config file on the same run

        // FIXME: handle errors gracefully
        let file_config = std::fs::read_to_string(&self.file)?;

        let config: RepartConfig = serde_systemd_unit::from_str(&file_config)?;

        tracing::trace!("{:#?}", config);

        // get fs type
        let fs_fmt = &config.partition.format;

        // serialize fs into string, if it's not there, use the fallback
        // use serde::Serialize;
        let fs_fmt_str = serde_json::to_string(fs_fmt)
            .unwrap_or_else(|_| FALLBACK_FS.to_owned())
            .replace('"', "");

        let mut mount_opts = String::new();

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

    // XXX: This is kinda weird.
    pub fn mount_point(&self) -> color_eyre::Result<Vec<(String, Option<String>)>> {
        // Read the config file or guess from DDI or return None
        let ddi_mountpoint = self.ddi_mountpoint();
        let label = self.label.clone();
        tracing::trace!(?ddi_mountpoint, ?label, "Reading mountpoint from DDI");

        let file_config = std::fs::read_to_string(&self.file)?;
        let config: RepartConfig = serde_systemd_unit::from_str(&file_config)?;

        tracing::trace!(?config, "Reading mountpoint from config file");

        let mut it = config
            .partition
            .mount_point_as_tuple()
            .into_iter()
            .peekable();
        if it.peek().is_some() {
            Ok(it.collect())
        } else if let Some(mntpoint) = self.ddi_mountpoint() {
            Ok(vec![(mntpoint.to_owned(), None)])
        } else {
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use super::*;
    const OUTPUT_EXAMPLE: &str = include_str!("repart-out.json");

    fn deserialize() -> RepartOutput {
        let val = serde_json::from_str(OUTPUT_EXAMPLE).unwrap();
        // println!("{:#?}", val);
        let output: RepartOutput = val;
        println!("{output:#?}");
        output
    }

    fn partition_fixture() -> RepartPartition {
        RepartPartition {
            part_type: "root-x86-64".to_owned(),
            label: "root-x86-64".to_owned(),
            uuid: uuid::Uuid::nil(),
            partno: 1,
            file: PathBuf::from("templates/wholedisk/50-root.conf"),
            ..Default::default()
        }
    }

    #[test]
    #[traced_test]
    fn test_ddi_mountpoint() {
        let part = partition_fixture();

        tracing::info!(?part, "Testing DDI mountpoint");

        assert_eq!(part.ddi_mountpoint(), Some("/"));

        // part
    }

    #[test]
    #[traced_test]
    fn root_fstab_entry() {
        let fake_output = RepartOutput {
            partitions: vec![partition_fixture()],
        };

        let fstab = fake_output.generate_fstab();

        tracing::info!(?fstab);

        assert!(fstab.unwrap().contains('/'));
    }

    #[test]
    fn test_deserialize() {
        let output = deserialize();
        assert_eq!(output.partitions.len(), 3);
    }

    #[test]
    #[traced_test]
    fn test_mountpoints() {
        let output = deserialize();
        let mountpoints = output
            .mountpoints()
            .collect::<std::collections::BTreeMap<_, _>>();
        println!("{mountpoints:#?}");
        assert_eq!(mountpoints.len(), 3);
        assert!(mountpoints.contains_key("/"));
        // assert_eq!(mountpoints.get("/boot"), Some(&"/dev/sda3".to_owned()));
        // assert_eq!(mountpoints.get("/boot/efi"), Some(&"/dev/sda1".to_owned()));
        // assert_eq!(mountpoints.get("/"), Some(&"/dev/sda4".to_owned()));
    }

    #[test]
    #[traced_test]
    #[ignore = "Requires the config files to be present in the root context"]
    fn test_fstab() {
        let output = deserialize();
        let mountpoints = output.generate_fstab().unwrap();

        println!("{mountpoints}");
    }
}
