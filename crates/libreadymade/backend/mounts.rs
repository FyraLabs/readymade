use crate::prelude::*;

use std::{
    cell::OnceCell,
    collections::HashMap,
    fmt::Write,
    fs::create_dir_all,
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use gpt::partition_types;
use nix::mount::umount;
use sys_mount::MountFlags;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EncryptionOption {
    // TODO: document
    KeyFile,
    Tpm2,
    KeyFileTpm2,
}

#[derive(Debug, Clone, Serialize, Deserialize, educe::Educe)]
#[educe(PartialEq, Eq)]
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
    #[educe(PartialEq(ignore))]
    gpt_type: OnceCell<gpt::partition_types::Type>,
}

/// Split fstab-style mount options into the flags understood by `mount(2)` and the
/// filesystem-specific data string.
///
/// Options that only make sense in fstab (`defaults`, `nofail`, `x-*`, …) are dropped, since
/// passing them as mount data makes the kernel reject the mount.
#[must_use]
pub fn split_mount_options(options: &str) -> (MountFlags, String) {
    let mut flags = MountFlags::empty();
    let mut data = vec![];
    for opt in options.split(',').map(str::trim).filter(|o| !o.is_empty()) {
        match opt {
            "ro" => flags |= MountFlags::RDONLY,
            "noatime" => flags |= MountFlags::NOATIME,
            "nodiratime" => flags |= MountFlags::NODIRATIME,
            "relatime" => flags |= MountFlags::RELATIME,
            "strictatime" => flags |= MountFlags::STRICTATIME,
            "nodev" => flags |= MountFlags::NODEV,
            "nosuid" => flags |= MountFlags::NOSUID,
            "noexec" => flags |= MountFlags::NOEXEC,
            "sync" => flags |= MountFlags::SYNCHRONOUS,
            "dirsync" => flags |= MountFlags::DIRSYNC,
            "defaults" | "rw" | "auto" | "noauto" | "nofail" | "user" | "nouser" | "users"
            | "owner" | "group" | "_netdev" | "async" | "atime" | "diratime" | "dev" | "suid"
            | "exec" => {}
            o if o.starts_with("x-") || o.starts_with("comment=") => {}
            o => data.push(o),
        }
    }
    (flags, data.join(","))
}

impl Mount {
    #[must_use]
    pub fn new(
        partition: PathBuf,
        mountpoint: PathBuf,
        options: String,
        encryption_type: Option<EncryptionOption>,
        label: Option<String>,
    ) -> Self {
        Self {
            partition,
            mountpoint,
            options,
            encryption_type,
            label,
            gpt_type: OnceCell::default(),
        }
    }

    /// The path of this mount's target under `root`.
    ///
    /// `mountpoint` is absolute, and joining an absolute path onto `root` would discard `root`.
    fn target(&self, root: &Path) -> PathBuf {
        root.join(
            self.mountpoint
                .strip_prefix("/")
                .unwrap_or(&self.mountpoint),
        )
    }

    /// The device to mount: the partition itself, or its decrypted mapper device if the
    /// partition is encrypted. Encrypted partitions are opened if they haven't been already.
    ///
    /// # Errors
    ///
    /// Returns an error if the partition is encrypted and no passphrase was given, or it cannot
    /// be decrypted.
    pub fn source(&self, passphrase: Option<&str>) -> Result<PathBuf> {
        if self.encryption_type.is_none() {
            return Ok(self.partition.clone());
        }
        let node = self.partition.to_string_lossy();
        if let Some(mapper) = MAPPER_CACHE.read().get(&*node) {
            return Ok(mapper.clone());
        }
        let passphrase = passphrase.ok_or_else(|| {
            eyre!("{node} is encrypted, but no passphrase was provided to decrypt it")
        })?;
        let label = generate_unique_mapper_label(&self.mountpoint.to_string_lossy());
        luks_decrypt(&node, passphrase, &label)
    }

    fn mount(&self, root: &Path, passphrase: Option<&str>) -> Result<()> {
        let target = self.target(root);

        tracing::info!(?root, "Mounting {:?} to {target:?}", self.partition);

        create_dir_all(&target)?;

        let source = self.source(passphrase)?;
        let (flags, data) = split_mount_options(&self.options);

        let mut builder = sys_mount::Mount::builder().flags(flags);
        if !data.is_empty() {
            builder = builder.data(&data);
        }
        builder.mount(&source, &target).with_context(|| {
            format!(
                "cannot mount from {} to {}",
                source.display(),
                target.display()
            )
        })?;

        Ok(())
    }

    /// Unmount this target from under `root`.
    ///
    /// # Errors
    ///
    /// Returns an error if the unmount fails.
    pub fn umount(&self, root: &Path) -> std::io::Result<()> {
        umount(&self.target(root))?;
        Ok(())
    }

    /// Get the GPT partition type of this mount's partition, reading it from disk on first use.
    ///
    /// # Errors
    ///
    /// Returns an error if the partition, its parent disk or its GPT entry cannot be found or read.
    pub fn get_gpt_type(&self) -> Result<gpt::partition_types::Type> {
        if let Some(t) = self.gpt_type.get() {
            return Ok(t.clone());
        }
        let part = lsblk::BlockDevice::from_path(&self.partition)
            .wrap_err_with(|| format!("cannot find block device {}", self.partition.display()))?;
        let disk = part.disk_name().map_err(|e| {
            eyre!(
                "cannot get parent disk of {}: {e:?}",
                self.partition.display()
            )
        })?;
        let partuuid = (part.partuuid.as_deref())
            .ok_or_else(|| eyre!("{} has no PARTUUID", self.partition.display()))?;
        let partuuid = Uuid::from_str(partuuid)?;

        let disk = gpt::disk::read_disk(format!("/dev/{disk}"))
            .wrap_err_with(|| format!("cannot read GPT of /dev/{disk}"))?;
        let part_type = (disk.partitions().values())
            .find(|p| p.part_guid == partuuid)
            .ok_or_else(|| eyre!("cannot find {} in the GPT", self.partition.display()))?
            .part_type_guid
            .clone();

        Ok(self.gpt_type.get_or_init(|| part_type).clone())
    }

    /// Whether this mount's partition has the GPT type `t`. Errors are logged and treated as
    /// a mismatch.
    fn has_gpt_type(&self, t: &partition_types::Type) -> bool {
        self.get_gpt_type()
            .inspect_err(|e| {
                tracing::warn!(?e, partition = ?self.partition, "cannot get GPT partition type");
            })
            .is_ok_and(|gpt_type| gpt_type == *t)
    }
}

/// Unmount `mounts` from under `root` in reverse, attempting every target even if one fails.
/// Errors are logged, and the first one is returned.
fn umount_rev<'a>(
    mounts: impl DoubleEndedIterator<Item = &'a Mount>,
    root: &Path,
) -> std::io::Result<()> {
    let mut res = Ok(());
    for m in mounts.rev() {
        let Err(e) = m.umount(root) else { continue };
        tracing::error!(?e, mountpoint = ?m.mountpoint, "Cannot unmount");
        if res.is_ok() {
            res = Err(e);
        }
    }
    res
}

/// Opened LUKS devices, keyed by the encrypted partition, so they can be reused and closed later.
// global cache, so we can clean up these devices later
pub static MAPPER_CACHE: std::sync::LazyLock<parking_lot::RwLock<HashMap<String, PathBuf>>> =
    std::sync::LazyLock::new(Default::default);

/// Close the LUKS mapper device `mapper`.
///
/// # Errors
///
/// Returns an error if `cryptsetup close` fails to run or exits unsuccessfully.
pub fn cryptsetup_close(mapper: &Path) -> Result<(), color_eyre::eyre::Error> {
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

/// Close every LUKS device opened by [`luks_decrypt`]. Failures are logged and skipped.
///
/// Everything mounted from the mapper devices must have been unmounted first.
pub fn close_all_mappers() {
    for (node, mapper) in MAPPER_CACHE.write().drain() {
        tracing::debug!(?node, ?mapper, "Closing LUKS device");
        if let Err(e) = cryptsetup_close(&mapper) {
            tracing::warn!(?e, ?node, ?mapper, "Cannot close LUKS device");
        }
    }
}

/// Open the LUKS partition `node` as `/dev/mapper/{label}` and return the mapper path.
///
/// Opened devices are cached in [`MAPPER_CACHE`], so decrypting the same node again returns the
/// existing mapper path.
///
/// # Errors
///
/// Returns an error if the key file cannot be written, or `cryptsetup open` fails.
#[allow(clippy::significant_drop_tightening)] // the lock is held on purpose
pub fn luks_decrypt(node: &str, passphrase: &str, label: &str) -> color_eyre::Result<PathBuf> {
    // Hold the write lock throughout so the same node can't be opened twice
    let mut cache = MAPPER_CACHE.write();
    if let Some(path) = cache.get(node) {
        return Ok(path.clone());
    }

    tracing::debug!(?node, ?label, "Decrypting LUKS partition");
    let mut key_file = tempfile::NamedTempFile::new()?;
    std::io::Write::write_all(&mut key_file, passphrase.as_bytes())
        .wrap_err("cannot write to key file")?;
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
    cache.insert(node.to_owned(), mapper.clone());

    Ok(mapper)
}

#[must_use]
pub fn generate_unique_mapper_label(mntpoint: &str) -> String {
    let mut label = {
        if mntpoint == "/" {
            "root".to_owned()
        } else {
            mntpoint.trim_matches('/').replace('/', "-")
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
    ///
    /// If a target fails to mount, the targets mounted before it are unmounted again.
    ///
    /// # Errors
    ///
    /// Returns an error if any target fails to be created, decrypted or mounted.
    pub fn mount_all(&self, root: &Path, passphrase: Option<&str>) -> Result<()> {
        for (i, m) in self.0.iter().enumerate() {
            if let Err(e) = m.mount(root, passphrase) {
                // errors are logged by umount_rev
                _ = umount_rev(self.0.iter().take(i), root);
                return Err(e);
            }
        }
        Ok(())
    }

    /// Unmount all the targets in reverse.
    ///
    /// Every target is attempted even if unmounting an earlier one fails.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered when unmounting.
    pub fn umount_all(&self, root: &Path) -> std::io::Result<()> {
        umount_rev(self.0.iter(), root)
    }

    /// Get the ESP partition if it exists
    ///
    /// This is a convenience function for getting the ESP partition, which we can then use for creating
    /// the boot stub later on
    ///
    /// Partitions whose GPT type cannot be read are skipped.
    #[must_use]
    pub fn get_esp_partition(&self) -> std::option::Option<&Mount> {
        self.0
            .iter()
            .find(|part| part.has_gpt_type(&partition_types::EFI))
    }

    /// Get the XBOOTLDR partition if it exists
    ///
    /// Partitions whose GPT type cannot be read are skipped.
    #[must_use]
    pub fn get_xbootldr_partition(&self) -> std::option::Option<&Mount> {
        const XBOOTLDR: Uuid = uuid::uuid!("bc13c2ff-59e6-4262-a352-b275fd6f7172");
        let xbootldr = partition_types::Type::from(XBOOTLDR);
        self.0.iter().find(|part| part.has_gpt_type(&xbootldr))
    }

    #[must_use]
    pub fn get_boot_partition(&self) -> std::option::Option<&Mount> {
        self.0
            .iter()
            .find(|part| part.mountpoint == Path::new("/boot"))
    }

    #[must_use]
    pub fn get_root_partition(&self) -> std::option::Option<&Mount> {
        self.0.iter().find(|part| part.mountpoint == Path::new("/"))
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

/// Check whether `node` is a LUKS device.
///
/// If `cryptsetup` cannot be run, this logs an error and returns `false`.
#[must_use]
pub fn is_luks(node: &Path) -> bool {
    Command::new("cryptsetup")
        .arg("isLuks")
        .arg(node)
        .status()
        .inspect_err(|e| tracing::error!(?e, "cannot run `cryptsetup isLuks`"))
        .is_ok_and(|status| status.success())
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

/// Generate the crypttab and kernel cmdline options for the LUKS partitions in `mounts`, or
/// `None` if there are none.
///
/// Partitions mounted more than once (e.g. btrfs subvolumes) only get a single entry. The
/// mapper name is the partition label, or `luks-<UUID>` if it has none.
///
/// # Errors
///
/// Returns an error if a LUKS partition's UUID cannot be obtained.
pub fn generate_cryptdata(mounts: &Mounts) -> Result<Option<CryptData>, color_eyre::eyre::Error> {
    // NOTE: https://www.man7.org/linux/man-pages/man5/crypttab.5.html
    let mut crypttab = String::new();
    let mut cmdline_opts = vec![];
    let mut is_tpm = false;
    let mut has_luks = false;

    let luks_partitions = (mounts.0.iter())
        .unique_by(|part| &part.partition)
        .filter(|part| is_luks(&part.partition));

    for part in luks_partitions {
        has_luks = true;
        let uuid = cryptsetup_luks_uuid(&part.partition)
            .wrap_err_with(|| format!("cannot get LUKS UUID of {}", part.partition.display()))?;
        let label = (part.label.clone())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| format!("luks-{uuid}"));

        let mut extra_opts = String::new();

        let part_uses_tpm = matches!(
            part.encryption_type,
            Some(EncryptionOption::KeyFileTpm2 | EncryptionOption::Tpm2)
        );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_mount_options() {
        assert_eq!(
            split_mount_options(""),
            (MountFlags::empty(), String::new())
        );
        assert_eq!(
            split_mount_options("defaults"),
            (MountFlags::empty(), String::new())
        );
        assert_eq!(
            split_mount_options("subvol=/,compress=zstd:1"),
            (MountFlags::empty(), "subvol=/,compress=zstd:1".to_owned())
        );
        assert_eq!(
            split_mount_options("defaults,noatime,ro,umask=0077,x-systemd.automount,nofail"),
            (
                MountFlags::NOATIME | MountFlags::RDONLY,
                "umask=0077".to_owned()
            )
        );
    }

    #[test]
    fn test_mount_target_keeps_root() {
        let m = Mount::new(
            "/dev/sda1".into(),
            "/boot/efi".into(),
            String::new(),
            None,
            None,
        );
        assert_eq!(m.target(Path::new("/mnt")), Path::new("/mnt/boot/efi"));
        let m = Mount::new("/dev/sda2".into(), "/".into(), String::new(), None, None);
        assert_eq!(m.target(Path::new("/mnt")), Path::new("/mnt"));
    }
}
