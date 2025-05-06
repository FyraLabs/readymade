use bytesize::ByteSize;
use color_eyre::eyre::{Context, OptionExt};
use std::path::PathBuf;
use std::{fmt::Write, sync::Arc};
use sys_mount::MountFlags;
use tiffin::{Container, MountTarget};

use crate::{
    backend::repartcfg::{FileSystem, RepartConfig},
    util::sys::check_uefi,
};

/// Wrapper struct for encryption data, so we don't have to pass around multiple
/// arguments
#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone)]
pub struct CryptData {
    /// Contents of /etc/crypttab
    pub crypttab: String,

    /// Extra cmdline options for the kernel
    pub cmdline_opts: Vec<String>,
    pub tpm: bool,
}

fn cryptsetup_luks_uuid(node: &str) -> Result<String, color_eyre::eyre::Error> {
    let cmd = std::process::Command::new("cryptsetup")
        .arg("luksUUID")
        .arg(node)
        .output()?;
    if !cmd.status.success() {
        color_eyre::eyre::bail!(
            "cryptsetup failed: {}",
            String::from_utf8_lossy(&cmd.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&cmd.stdout).trim().to_owned())
}

pub struct MapperCache {
    cache: std::collections::HashMap<String, PathBuf>,
}

impl MapperCache {
    fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn get(&self, node: &str) -> Option<&PathBuf> {
        self.cache.get(node)
    }

    pub(crate) fn insert(&mut self, node: String, path: PathBuf) {
        self.cache.insert(node, path);
    }

    pub(crate) fn clear(&mut self) {
        for (node, path) in self.cache.drain() {
            if let Err(e) = cryptsetup_close(&path.to_string_lossy()) {
                tracing::error!(?node, ?e, "Failed to close mapper device");
            }
        }
    }
}

pub fn cryptsetup_close(mapper: &str) -> Result<(), color_eyre::eyre::Error> {
    let cmd = std::process::Command::new("cryptsetup")
        .arg("close")
        .arg(mapper)
        .output()?;
    if !cmd.status.success() {
        color_eyre::eyre::bail!(
            "cryptsetup failed: {}",
            String::from_utf8_lossy(&cmd.stderr)
        );
    }
    Ok(())
}

pub fn generate_unique_mapper_label(mntpoint: &str) -> String {
    let mut label = {
        if mntpoint == "/" {
            "root".to_owned()
        } else {
            mntpoint.trim_start_matches('/').replace('/', "-")
        }
    };

    // Check if mapper device already exists and append counter if needed
    let mut counter = 0;
    let base_label = label.clone();
    while std::path::Path::new(&format!("/dev/mapper/{label}")).exists() {
        counter += 1;
        label = format!("{base_label}-{counter}");
    }
    label
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
        (self.partitions.iter())
            .filter_map(|part| part.ddi_mountpoint().map(|mp| (mp, part.node.clone())))
    }

    #[allow(clippy::unwrap_in_result)]
    /// # Panics
    /// if LUKS UUID cannot be obtained.
    pub fn generate_cryptdata(&self) -> Result<Option<CryptData>, color_eyre::eyre::Error> {
        // NOTE: https://www.man7.org/linux/man-pages/man5/crypttab.5.html
        let mut crypttab = String::new();
        let mut cmdline_opts = vec![];

        let luks_partitions = self.partitions.iter().filter(|part| is_luks(&part.node));

        let mut is_tpm = false;

        let has_luks = luks_partitions.clone().count() > 0;
        for part in luks_partitions {
            let uuid = cryptsetup_luks_uuid(&part.node).expect("Failed to get LUKS UUID");
            let label = &part.label;

            let mut extra_opts = String::new();

            let part_uses_tpm: bool = {
                let file_config = std::fs::read_to_string(&part.file)?;
                let config: RepartConfig = serde_systemd_unit::from_str(&file_config)?;

                matches!(
                    config.partition.encrypt,
                    super::repartcfg::EncryptOption::KeyFileTpm2
                        | super::repartcfg::EncryptOption::Tpm2
                )
            };

            if part_uses_tpm {
                is_tpm = true;
                extra_opts.push_str("tpm2-device=auto,");
            }

            writeln!(
                &mut crypttab,
                "{label}\tUUID={uuid}\tnone\t{extra_opts}luks,discard"
            )?;

            cmdline_opts.push(format!("rd.luks.name={uuid}={label}"));
        }

        if is_tpm {
            cmdline_opts.push("rd.luks.options=tpm2-device=auto".to_owned());
        }

        Ok(has_luks.then_some(CryptData {
            crypttab,
            cmdline_opts,
            tpm: is_tpm,
        }))
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

    /// Generates a unique label for a mapper device based on a mount point
    ///
    /// This function takes a mount point and generates a unique label for the mapper device by:
    /// 1. Converting the mount point to a valid label format
    /// 2. Checking if that label is already in use and appending a counter if needed
    ///
    /// Create `tiffin::Container` from the repartitioning output with the mountpoints
    /// from the DDI partition types
    pub fn to_container(&self, passphrase: Option<&str>) -> color_eyre::Result<Container> {
        tracing::info!("Creating container from repartitioning output");
        let temp_dir = tempfile::tempdir()?.into_path();
        // A table of decrypted partitions, so we don't have to decrypt the same partition multiple times
        let mut decrypted_partitions: std::collections::HashMap<String, PathBuf> =
            std::collections::HashMap::new();

        // let config: RepartConfig = serde_systemd_unit::from_str(&file_config)?;

        let mut container = Container::new(temp_dir);

        for (mntpoint, node) in self.mountpoints() {
            let node = if is_luks(&node) {
                if let Some(mapper) = decrypted_partitions.get(&node) {
                    mapper.clone()
                } else {
                    let pass =
                        passphrase.ok_or_eyre("Passphrase is empty when is_luks() is true")?;
                    // We need to sanitize the label for the mapper device name, as it can't contain slashes
                    //
                    // I forgot to account for this when I refactored it -Cappy
                    //
                    let label = generate_unique_mapper_label(mntpoint);
                    // XXX: This introduces some weird ordering issues with generate_fstab when decrypting from here
                    // Because generate_fstab() assumes that the partitions are decrypted already.
                    //
                    // todo: add some global cache for decrypted partitions
                    let mapper = luks_decrypt(&node, pass, &label)?;
                    decrypted_partitions.insert(node.clone(), mapper.clone());
                    mapper
                }
            } else {
                PathBuf::from(node)
            };
            // pass in mount opts?
            let mnt_target = MountTarget {
                target: PathBuf::from(mntpoint),
                flags: MountFlags::empty(),
                data: None,
                fstype: None,
            };
            container.add_mount(mnt_target, node);
        }

        // end experimental path

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

        container.host_bind_mount();
        container.bind_mount(PathBuf::from("/run/host/usr"), PathBuf::from("/usr"));
        container.bind_mount(PathBuf::from("/run/host/etc"), PathBuf::from("/etc"));
        container.bind_mount(PathBuf::from("/run/host/lib"), PathBuf::from("/lib"));
        container.bind_mount(PathBuf::from("/run/host/lib64"), PathBuf::from("/lib64"));
        container.bind_mount(PathBuf::from("/run/host/bin"), PathBuf::from("/bin"));

        Ok(container)
    }
}

pub fn is_luks(node: &str) -> bool {
    std::process::Command::new("cryptsetup")
        .args(["isLuks", node])
        .status()
        .expect("cannot run cryptsetup")
        .success()
}

// global cache, so we can clean up these devices later
pub static MAPPER_CACHE: std::sync::LazyLock<parking_lot::RwLock<Arc<MapperCache>>> =
    std::sync::LazyLock::new(|| parking_lot::RwLock::new(Arc::new(MapperCache::new())));

pub fn luks_decrypt(
    node: &str,
    passphrase: &str,
    label: &str,
) -> Result<PathBuf, color_eyre::eyre::Error> {
    // Check cache first
    if let Some(path) = MAPPER_CACHE.read().get(node) {
        return Ok(path.clone());
    }

    tracing::debug!("Decrypting LUKS partition");
    let temp_dir = tempfile::tempdir()?.into_path();
    let key_file_path = temp_dir.join("keyfile.txt");
    let mut key_file = std::fs::File::create(&key_file_path).wrap_err("cannot create key file")?;
    std::io::Write::write_all(&mut key_file, passphrase.as_bytes())
        .wrap_err("cannot write to key file")?;
    drop(key_file);
    // i guess to_container now also needs to accept an Option<String> for the passphrase
    let cmd = std::process::Command::new("cryptsetup")
        .args(["open", node, label, "--batch-mode", "--key-file"])
        .arg(key_file_path)
        .output()?;

    if !cmd.status.success() {
        color_eyre::eyre::bail!(
            "cryptsetup failed: {}",
            String::from_utf8_lossy(&cmd.stderr)
        );
    }

    let mapper = PathBuf::from(format!("/dev/mapper/{label}"));

    // Add to cache
    Arc::get_mut(&mut *MAPPER_CACHE.write())
        .unwrap()
        .insert(node.to_owned(), mapper.clone());

    Ok(mapper)
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

        tracing::trace!("{config:#?}");

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
            FALLBACK_OPTS.clone_into(&mut mount_opts);
        }

        // let's get the UUID

        // let uuid = self.uuid.to_string();
        // Check if the disk is encrypted
        let is_encrypted = is_luks(&self.node);
        let uuid_string = if is_encrypted {
            tracing::trace!("Partition is encrypted");
            // We're gonna do what's called a pro gamer move.
            // HACK: We will guess the UUID of the decrypted LUKS partition by:
            // - Guessing where the mapper device will be
            // - Finding the UUID of the mapper device by doing some symlink magic (thanks udev!)
            // - Using that UUID for the fstab entry

            // We're gonna be abusing the mapper cache, which should be populated by the time we get here

            let mapper_cache = MAPPER_CACHE.read();
            let mapper_path = mapper_cache.get(&self.node).unwrap();

            tracing::trace!(?mapper_path, "Guessed mapper path as this");

            // Thankfully, since we made lsblk-rs we can do this easily.
            let device = lsblk::BlockDevice::from_path(mapper_path)?;
            drop(mapper_cache);
            tracing::trace!(?device, "Found device from mapper path");
            let uuid = device
                .uuid
                .ok_or_eyre("Could not find UUID of decrypted device")?;

            // The mapper path should be a symlink to the /dev/dm-XX device

            // let dm = std::fs::read_link(&mapper_path)?;

            // tracing::trace!(?dm, "Found decrypted device");

            // let uuid = std::fs::read_dir("/dev/disk/by-uuid")?
            //     .find_map(|entry| {
            //         let entry = entry.ok()?;
            //         let path = entry.path();
            //         let link = std::fs::read_link(&path).ok()?;
            //         if link == dm {
            //             Some(path.file_name()?.to_string_lossy().to_string())
            //         } else {
            //             None
            //         }
            //     })
            //     .ok_or_eyre("Could not find UUID for decrypted device")?;

            // tracing::trace!(?uuid, "Found UUID for decrypted device!");

            format!("UUID={uuid}")
        } else {
            tracing::trace!("Partition is not encrypted, using repart's");
            format!("PARTUUID={}", self.uuid)
        };

        // let's get the dump and pass values

        let dump = FALLBACK_DUMP; //todo: is there a config option for this?

        // We will be checking from filesystem type
        // or the root device it should be 1. For other partitions it should be 2, or 0 to disable checking.
        // If the root file system is btrfs or XFS, the fsck order should be set to 0 instead of 1.
        let pass = match fs_fmt {
            Some(FileSystem::Btrfs | FileSystem::Xfs) => 0,
            _ if mntpoint == "/" => 1,
            _ => FALLBACK_PASS,
        };

        // now let's write the fstab entry
        Ok(format!(
            "{uuid_string}\t{mntpoint}\t{fs_fmt_str}\t{mount_opts}\t{dump}\t{pass}"
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

        let mps = config.partition.mount_point_as_tuple();
        Ok(if !mps.is_empty() {
            mps
        } else if let Some(mntpoint) = self.ddi_mountpoint() {
            vec![(mntpoint.to_owned(), None)]
        } else {
            vec![]
        })
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
