use crate::prelude::*;

use std::{
    cell::OnceCell,
    fmt::Write,
    fs::create_dir_all,
    path::{Component, Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use gpt::partition_types;
use nix::mount::umount;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EncryptionOption {
    // TODO: document
    KeyFile,
    Tpm2,
    KeyFileTpm2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mount {
    /// Path to partition
    pub partition: PathBuf,
    /// Path to mountpoint
    pub mountpoint: PathBuf,
    /// Raw text `mountopts`
    pub options: String,
    /// Encryption type of the partition, if any
    pub encryption_type: Option<EncryptionOption>,
    /// Label of the partition
    pub label: Option<String>,
    /// GPT Partition type (assume we only support GPT)
    // TODO: make this a method? / private
    #[serde(skip)]
    pub(crate) gpt_type: OnceCell<gpt::partition_types::Type>,
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

// global cache, so we can clean up these devices later
pub static MAPPER_CACHE: std::sync::LazyLock<parking_lot::RwLock<Arc<MapperCache>>> =
    std::sync::LazyLock::new(|| parking_lot::RwLock::new(Arc::new(MapperCache::new())));

pub fn luks_decrypt(node: &str, passphrase: &str, label: &str) -> color_eyre::Result<PathBuf> {
    // Check cache first
    if let Some(path) = MAPPER_CACHE.read().get(node) {
        return Ok(path.clone());
    }

    tracing::debug!("Decrypting LUKS partition");
    let mut key_file = tempfile::NamedTempFile::new()?;
    std::io::Write::write_all(&mut key_file, passphrase.as_bytes())
        .wrap_err("cannot write to key file")?;
    // i guess to_container now also needs to accept an Option<String> for the passphrase
    let cmd = std::process::Command::new("cryptsetup")
        .args(["open", node, label, "--batch-mode", "--key-file"])
        .arg(key_file.path())
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

#[must_use]
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

impl Mount {
    fn mount(&self, root: &Path, passphrase: Option<&str>) -> Result<()> {
        create_dir_all(root)?;

        let target = (self.mountpoint.strip_prefix("/")).unwrap_or(&self.mountpoint);

        tracing::info!(?root, "Mounting {:?} to {target:?}", self.partition);

        let target = root.join(target);
        create_dir_all(&target)?;

        let source = if let Some(_) = self.encryption_type {
            let label =
                generate_unique_mapper_label(format!("{}", self.mountpoint.display()).as_str());
            &luks_decrypt(
                format!("{}", self.partition.display()).as_str(),
                passphrase.unwrap(),
                &label,
            )?
        } else {
            &self.partition
        };

        sys_mount::Mount::builder()
            .data(&self.options)
            .mount(&source, target)?;

        Ok(())
    }

    pub fn umount(&self, root: &Path) -> std::io::Result<()> {
        // sanitize target path
        let target = (self.mountpoint.strip_prefix("/")).unwrap_or(&self.mountpoint);
        let target = root.join(target);

        umount(&target)?;
        Ok(())
    }

    pub fn get_gpt_type(&self) -> gpt::partition_types::Type {
        self.gpt_type
            .get_or_init(|| {
                let part = lsblk::BlockDevice::from_path(&self.partition).unwrap();
                let parent = format!("/dev/{}", part.disk_name().unwrap());

                let partitions = gpt::disk::read_disk(&parent).unwrap();
                let partitions = partitions.partitions();

                let partition = partitions
                    .iter()
                    .map(|(_, p)| p)
                    .find(|p| {
                        Some(p.part_guid)
                            == part.partuuid.as_ref().map(|u| Uuid::from_str(&u).unwrap())
                    })
                    .expect("cannot find partition that is supposed to exist");

                partition.part_type_guid.clone()
            })
            .clone()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Mounts(pub Vec<Mount>);

impl Mounts {
    //? https://github.com/FyraLabs/tiffin/blob/3d09faf3127f644fbd441af78d039b1acaba5847/src/lib.rs#L117C1-L130C6
    /// Sort mounts by mountpoint and depth
    /// Closer to root, and root is first
    /// everything else is either sorted by depth, or alphabetically
    pub fn sort_mounts(&mut self) {
        self.0.sort_by(|a, b| {
            match (
                a.mountpoint.components().count(),
                b.mountpoint.components().count(),
            ) {
                (1, _) if a.mountpoint.components().next() == Some(Component::RootDir) => {
                    std::cmp::Ordering::Less
                } // root dir
                (_, 1) if b.mountpoint.components().next() == Some(Component::RootDir) => {
                    std::cmp::Ordering::Greater
                } // root dir
                (x, y) if x == y => a.mountpoint.cmp(&b.mountpoint),
                (x, y) => x.cmp(&y),
            }
        });
    }

    /// Mount all the targets in the specified order.
    pub fn mount_all(&self, root: &Path, passphrase: Option<&str>) -> Result<()> {
        self.0.iter().try_for_each(|m| m.mount(root, passphrase))
    }

    /// Unmount all the targets in reverse.
    pub fn umount_all(&self, root: &Path) -> std::io::Result<()> {
        self.0.iter().rev().try_for_each(|m| m.umount(root))
    }

    /// Get the ESP partition if it exists
    ///
    /// This is a convenience function for getting the ESP partition, which we can then use for creating
    /// the boot stub later on
    #[must_use]
    pub fn get_esp_partition(&self) -> std::option::Option<&Mount> {
        self.0
            .iter()
            .find(|part| part.get_gpt_type() == partition_types::EFI)
    }

    #[must_use]
    pub fn get_xbootldr_partition(&self) -> std::option::Option<&Mount> {
        self.0.iter().find(|part| {
            part.get_gpt_type()
                == partition_types::Type::from(
                    Uuid::from_str("bc13c2ff-59e6-4262-a352-b275fd6f7172").unwrap(),
                )
        })
    }
}

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

#[must_use]
pub fn is_luks(node: &Path) -> bool {
    Command::new("cryptsetup")
        .arg("isLuks")
        .arg(node)
        .status()
        .expect("cannot run cryptsetup")
        .success()
}
fn cryptsetup_luks_uuid(node: &Path) -> Result<String, color_eyre::eyre::Error> {
    let cmd = Command::new("cryptsetup")
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

#[allow(clippy::unwrap_in_result)]
/// # Panics
/// if LUKS UUID cannot be obtained.
pub fn generate_cryptdata(mounts: &Mounts) -> Result<Option<CryptData>, color_eyre::eyre::Error> {
    // NOTE: https://www.man7.org/linux/man-pages/man5/crypttab.5.html
    let mut crypttab = String::new();
    let mut cmdline_opts = vec![];

    let luks_partitions = mounts.0.iter().filter(|part| is_luks(&part.partition));

    let mut is_tpm = false;

    let has_luks = luks_partitions.clone().count() > 0;
    for part in luks_partitions {
        let uuid = cryptsetup_luks_uuid(&part.partition).expect("Failed to get LUKS UUID");
        let label = part
            .label
            .as_ref()
            .expect("LUKS partition must have a label");

        let mut extra_opts = String::new();

        let part_uses_tpm: bool = {
            matches!(
                part.encryption_type,
                Some(EncryptionOption::KeyFileTpm2 | EncryptionOption::Tpm2)
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
