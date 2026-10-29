use std::str::FromStr;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum FsTableError {
    #[error("Invalid fstab entry: {0}")]
    InvalidEntry(String),

    #[error("Invalid number conversion: {0}")]
    InvalidNumberConversion(String),

    #[error("Invalid fsck order: {0}")]
    InvalidFsckOrder(u8),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("lsblk error: {0}")]
    LsblkError(#[from] lsblk::LsblkError),
}

type Result<T> = std::result::Result<T, FsTableError>;

/// The order in which the filesystems should be checked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum FsckOrder {
    /// Never check the filesystem automatically.
    #[default]
    NoCheck = 0,
    /// Check the filesystem while booting.
    Boot = 1,
    /// Check the filesystem after the boot process has finished.
    PostBoot = 2,
}

impl TryFrom<&u8> for FsckOrder {
    type Error = FsTableError;

    fn try_from(value: &u8) -> Result<Self> {
        match value {
            0 => Ok(Self::NoCheck),
            1 => Ok(Self::Boot),
            2 => Ok(Self::PostBoot),
            _ => Err(FsTableError::InvalidFsckOrder(*value)),
        }
    }
}

impl TryFrom<u8> for FsckOrder {
    type Error = FsTableError;

    fn try_from(value: u8) -> Result<Self> {
        // use the TryFrom<&u8> implementation
        Self::try_from(&value)
    }
}

impl TryFrom<&str> for FsckOrder {
    type Error = FsTableError;

    fn try_from(value: &str) -> Result<Self> {
        let n = value
            .parse::<u8>()
            .map_err(|e| FsTableError::InvalidNumberConversion(e.to_string()))?;
        Self::try_from(n)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FsEntry {
    /// The device spec for mounting the filesystem.
    ///
    /// Can be a device path, or some kind of filter to get the
    /// device, i.e `LABEL=ROOT` or `UUID=1234-5678`
    ///
    /// Examples:
    ///
    /// - `/dev/sda1`
    /// - `LABEL=ROOT`
    /// - `UUID=1234-5678`
    /// - `PARTUUID=1234-5678`
    /// - `PARTLABEL=ROOT`
    pub device_spec: String,
    /// The mountpoint for the filesystem.
    /// Specifies where the filesystem should be mounted.
    ///
    /// Doesn't actually need to be a real mountpoint, but
    /// most of the time it will be.
    ///
    /// Is an optional field, a [`None`] value will serialize into `none`.
    ///
    /// Examples:
    ///
    /// - `/`
    /// - `/boot`
    /// - `none` (for no mountpoint, used for swap or similar filesystems)
    /// - `/home`
    pub mountpoint: Option<String>,

    /// The filesystem type for the filesystem.
    ///
    /// Examples:
    ///
    /// - `ext4`
    /// - `btrfs`
    /// - `vfat`
    /// - ...
    pub fs_type: String,

    /// Mount options for the filesystem. Is a comma-separated list of options.
    ///
    /// This type returns a vector of strings, as there can be multiple options.
    /// They will be serialized into a comma-separated list.
    pub options: Vec<String>,

    /// The dump frequency for the filesystem.
    ///
    /// This is a number that specifies how often the filesystem should be backed up.
    ///
    pub dump_freq: u8,

    /// The pass number for the filesystem.
    ///
    /// Determines when the filesystem health should be checked using `fsck`.
    pub pass: FsckOrder,
}

/// Characters that have to be escaped in fstab fields, as octal escapes like `\040`.
///
/// This matches what libmount and the kernel (for `/proc/mounts`) escape.
const ESCAPED_CHARS: [char; 4] = [' ', '\t', '\n', '\\'];

/// Escape a field for fstab, e.g. turning spaces into `\040`.
#[must_use]
pub fn escape_field(field: &str) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(field.len());
    for c in field.chars() {
        if ESCAPED_CHARS.contains(&c) {
            write!(out, "\\{:03o}", u32::from(c)).unwrap();
        } else {
            out.push(c);
        }
    }
    out
}

/// Undo [`escape_field`], turning octal escapes like `\040` back into characters.
///
/// Backslashes not followed by three octal digits are kept as-is.
#[must_use]
pub fn unescape_field(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut rest = field;
    while let Some(i) = rest.find('\\') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let escaped = rest
            .get(1..4)
            .filter(|digits| digits.bytes().all(|b| (b'0'..=b'7').contains(&b)))
            .and_then(|digits| u8::from_str_radix(digits, 8).ok());
        if let Some(byte) = escaped {
            out.push(char::from(byte));
            rest = &rest[4..];
        } else {
            out.push('\\');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

impl FsEntry {
    /// Parse a `FsEntry` from a line in the fstab file.
    ///
    /// As in `fstab(5)`, the dump frequency and fsck pass fields are optional and default to 0.
    /// Escape sequences such as `\040` in the device spec and mountpoint are decoded.
    ///
    /// # Errors
    ///
    /// Returns [`FsTableError::InvalidEntry`] if the line does not have 4 to 6 fields or the dump
    /// frequency is not a valid number, or an error if the fsck pass is invalid.
    pub fn from_line_str(line: &str) -> std::result::Result<Self, FsTableError> {
        // split by whitespace
        let parts: Vec<&str> = line.split_whitespace().collect();

        let (&[device_spec, mountpoint, fs_type, options], rest) =
            parts.split_at(4.min(parts.len()))
        else {
            return Err(FsTableError::InvalidEntry(line.to_owned()));
        };
        let (dump_freq, pass) = match *rest {
            [] => ("0", "0"),
            [dump_freq] => (dump_freq, "0"),
            [dump_freq, pass] => (dump_freq, pass),
            _ => return Err(FsTableError::InvalidEntry(line.to_owned())),
        };

        let device_spec = unescape_field(device_spec);

        let mountpoint = (mountpoint != "none").then(|| unescape_field(mountpoint));

        let fs_type = unescape_field(fs_type);

        let options = options.split(',').map(unescape_field).collect();

        let dump_freq = dump_freq
            .parse::<u8>()
            .map_err(|_| FsTableError::InvalidEntry(line.to_owned()))?;
        let pass = FsckOrder::try_from(pass)?;

        Ok(Self {
            device_spec,
            mountpoint,
            fs_type,
            options,
            dump_freq,
            pass,
        })
    }

    /// Serialize the `FsEntry` into a line (without a trailing newline) that can be written to
    /// the fstab file. Whitespace and backslashes in fields are escaped, e.g. as `\040`.
    #[must_use]
    pub fn to_line_str(&self) -> String {
        let mountpoint = self
            .mountpoint
            .as_deref()
            .map_or_else(|| "none".to_owned(), escape_field);
        let options = if self.options.is_empty() {
            "defaults".to_owned()
        } else {
            self.options
                .iter()
                .map(|o| escape_field(o))
                .collect::<Vec<_>>()
                .join(",")
        };

        format!(
            "{device_spec}\t{mountpoint}\t{fs_type}\t{options}\t{dump_freq}\t{pass}",
            device_spec = escape_field(&self.device_spec),
            fs_type = escape_field(&self.fs_type),
            dump_freq = self.dump_freq,
            pass = self.pass as u8,
        )
    }
}

impl TryFrom<&str> for FsEntry {
    type Error = FsTableError;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_line_str(value)
    }
}

impl std::fmt::Display for FsEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_line_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FsTable {
    pub entries: Vec<FsEntry>,
}

impl FromStr for FsTable {
    type Err = FsTableError;

    /// Parse a table, skipping blank lines and `#` comments.
    fn from_str(table: &str) -> Result<Self> {
        let entries = table
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(FsEntry::from_line_str)
            .collect::<Result<Vec<FsEntry>>>()?;

        Ok(Self { entries })
    }
}

impl std::fmt::Display for FsTable {
    /// Write the table out, one newline-terminated line per entry, ready to be written to a file.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for entry in &self.entries {
            writeln!(f, "{entry}")?;
        }
        Ok(())
    }
}

impl TryFrom<&str> for FsTable {
    type Error = FsTableError;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_str(value)
    }
}

/// Read and parse `/etc/mtab`.
///
/// # Errors
///
/// Returns an error if `/etc/mtab` cannot be read or contains an invalid entry.
pub fn read_mtab() -> Result<FsTable> {
    FsTable::from_str(&std::fs::read_to_string("/etc/mtab")?)
}

/// Read and parse `/etc/fstab`.
///
/// # Errors
///
/// Returns an error if `/etc/fstab` cannot be read or contains an invalid entry.
pub fn read_fstab() -> Result<FsTable> {
    FsTable::from_str(&std::fs::read_to_string("/etc/fstab")?)
}

/// Generate a new fstab from mtab, using a chroot prefix to generate the new fstab.
///
/// This is useful when you want to generate a new fstab for a chroot environment.
///
///
/// # Example
///
/// ```rust,no_run
/// let fstab = filesystem_table::generate_fstab("/mnt/custom").unwrap();
///
/// print!("{fstab}");
/// ```
///
/// This will generate a new fstab for the `/mnt/custom` chroot.
///
/// # Errors
///
/// Returns an error if `/etc/mtab` cannot be read or parsed, block devices cannot be listed,
/// or a mounted device under `prefix` has no UUID.
pub fn generate_fstab(prefix: &str) -> Result<FsTable> {
    fstab_from_mtab(read_mtab()?, &lsblk::BlockDevice::list()?, prefix)
}

/// Strip `prefix` from `path` if `path` is `prefix` itself or a path below it.
fn strip_mount_prefix(path: &str, prefix: &str) -> Option<String> {
    match path.strip_prefix(prefix)? {
        "" => Some("/".to_owned()),
        rest if rest.starts_with('/') || prefix.is_empty() => Some(rest.to_owned()),
        // e.g. `/mnt/customer` with prefix `/mnt/custom`
        _ => None,
    }
}

/// The logic behind [`generate_fstab`]: take the entries of `mtab` mounted under `prefix`, rebase
/// their mountpoints onto `prefix` and refer to their devices by UUID.
///
/// Entries that aren't backed by a block device (e.g. `proc` or `tmpfs`) are skipped.
fn fstab_from_mtab(
    mtab: FsTable,
    block_list: &[lsblk::BlockDevice],
    prefix: &str,
) -> Result<FsTable> {
    // if prefix ends with /, strip it
    //
    // This solves some common cases where the prefix contains a trailing slash,
    // causing only the subdirectories to be matched.
    let prefix = prefix.trim_end_matches('/');

    let entries = (mtab.entries.into_iter())
        .filter_map(|mut entry| {
            let mountpoint = strip_mount_prefix(entry.mountpoint.as_deref()?, prefix)?;
            entry.mountpoint = Some(mountpoint);
            Some(entry)
        })
        .filter(|entry| entry.device_spec.starts_with("/dev/"))
        .map(|mut entry| -> Result<FsEntry> {
            let device = std::path::Path::new(&entry.device_spec);
            // mtab may list e.g. `/dev/mapper/foo`, which lsblk resolves to `/dev/dm-0`
            let canonical = std::fs::canonicalize(device).ok();
            let uuid = block_list
                .iter()
                .find(|dev| dev.fullname == device || Some(&dev.fullname) == canonical.as_ref())
                .and_then(|dev| dev.uuid.as_ref())
                .ok_or_else(|| {
                    FsTableError::InvalidEntry(format!(
                        "Could not find UUID for device: {}",
                        entry.device_spec
                    ))
                })?;
            entry.device_spec = format!("UUID={uuid}");
            Ok(entry)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(FsTable { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fstab_parse() {
        let line = "/dev/sda1\t/\text4\trw,relatime\t0\t1";
        let entry = FsEntry::from_line_str(line).unwrap();

        assert_eq!(entry.device_spec, "/dev/sda1");
        assert_eq!(entry.mountpoint, Some("/".to_owned()));
        assert_eq!(entry.fs_type, "ext4");
        assert_eq!(entry.options, vec!["rw", "relatime"]);
        assert_eq!(entry.dump_freq, 0);
        assert_eq!(entry.pass, FsckOrder::Boot);
    }

    #[test]
    fn test_fstab_serialize() {
        let entry = FsEntry {
            device_spec: "/dev/sda1".to_owned(),
            mountpoint: Some("/".to_owned()),
            fs_type: "ext4".to_owned(),
            options: vec!["rw".to_owned(), "relatime".to_owned()],
            dump_freq: 0,
            pass: FsckOrder::Boot,
        };

        assert_eq!(entry.to_line_str(), "/dev/sda1\t/\text4\trw,relatime\t0\t1");
    }

    #[test]
    fn test_fsck_order() {
        assert_eq!(FsckOrder::try_from(&0u8).unwrap() as u8, 0);
        assert_eq!(FsckOrder::try_from(&1u8).unwrap() as u8, 1);
        assert_eq!(FsckOrder::try_from(&2u8).unwrap() as u8, 2);
        FsckOrder::try_from(&3u8).unwrap_err();
    }

    #[test]
    fn test_fstab_table() {
        let table = "/dev/sda1\t/\text4\trw,relatime\t0\t1\n/dev/sda2\tnone\tswap\tsw\t0\t0";
        let fstab = FsTable::from_str(table).unwrap();

        let [first, second] = fstab.entries.as_slice() else {
            panic!("expected 2 entries, got {}", fstab.entries.len());
        };
        assert_eq!(first.device_spec, "/dev/sda1");
        assert_eq!(second.device_spec, "/dev/sda2");

        let serialized = fstab.to_string();
        assert_eq!(serialized, format!("{table}\n"));
    }

    #[test]
    fn test_fstab_table_comments_and_optional_fields() {
        let table = "# /etc/fstab\n\n  # Created by anaconda\nUUID=abcd / btrfs subvol=root 0 0\n\
                     UUID=ef01 /boot ext4 defaults\n";
        let fstab = FsTable::from_str(table).unwrap();
        let [root, boot] = fstab.entries.as_slice() else {
            panic!("expected 2 entries, got {fstab:#?}");
        };
        assert_eq!(root.mountpoint.as_deref(), Some("/"));
        assert_eq!(boot.dump_freq, 0);
        assert_eq!(boot.pass, FsckOrder::NoCheck);

        FsTable::from_str("UUID=abcd / btrfs").unwrap_err();
        FsTable::from_str("UUID=abcd / btrfs defaults 0 0 extra").unwrap_err();
    }

    #[test]
    fn test_escaping() {
        let entry = FsEntry {
            device_spec: "LABEL=My Disk".to_owned(),
            mountpoint: Some("/mnt/my files\\x".to_owned()),
            fs_type: "ext4".to_owned(),
            options: vec!["defaults".to_owned()],
            dump_freq: 0,
            pass: FsckOrder::PostBoot,
        };
        let line = entry.to_line_str();
        assert_eq!(
            line,
            "LABEL=My\\040Disk\t/mnt/my\\040files\\134x\text4\tdefaults\t0\t2"
        );
        assert_eq!(FsEntry::from_line_str(&line).unwrap(), entry);

        assert_eq!(unescape_field("a\\011b\\012c"), "a\tb\nc");
        // not an octal escape
        assert_eq!(unescape_field("a\\9b\\"), "a\\9b\\");
    }

    fn block_device(fullname: &str, uuid: &str) -> lsblk::BlockDevice {
        lsblk::BlockDevice {
            name: fullname.trim_start_matches("/dev/").to_owned(),
            fullname: fullname.into(),
            uuid: Some(uuid.to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn test_fstab_from_mtab() {
        let mtab = FsTable::from_str(
            "/dev/vda3 / btrfs rw,relatime 0 0
proc /proc proc rw,nosuid 0 0
/dev/vda3 /mnt/custom btrfs rw,subvol=/root 0 0
/dev/vda3 /mnt/custom/home btrfs rw,subvol=/home 0 0
/dev/vda2 /mnt/custom/boot ext4 rw,relatime 0 0
proc /mnt/custom/proc proc rw 0 0
tmpfs /mnt/custom/tmp tmpfs rw 0 0
/dev/vda4 /mnt/customer ext4 rw 0 0
/dev/vda5 /mnt/custom/my\\040data xfs rw 0 0
",
        )
        .unwrap();
        let block_list = [
            block_device("/dev/vda2", "2222"),
            block_device("/dev/vda3", "3333"),
            block_device("/dev/vda4", "4444"),
            block_device("/dev/vda5", "5555"),
        ];

        let fstab = fstab_from_mtab(mtab.clone(), &block_list, "/mnt/custom/").unwrap();
        assert_eq!(
            fstab.to_string(),
            "UUID=3333\t/\tbtrfs\trw,subvol=/root\t0\t0
UUID=3333\t/home\tbtrfs\trw,subvol=/home\t0\t0
UUID=2222\t/boot\text4\trw,relatime\t0\t0
UUID=5555\t/my\\040data\txfs\trw\t0\t0
"
        );

        // a mounted block device we can't find a UUID for is an error
        fstab_from_mtab(mtab, &block_list[..1], "/mnt/custom").unwrap_err();
    }

    #[test]
    fn test_mtab_parse() {
        let mtab = std::fs::read_to_string("/etc/mtab").unwrap();

        let table = FsTable::from_str(&mtab).unwrap();
        assert_eq!(table.entries.len(), mtab.lines().count());
    }

    #[test]
    #[ignore = "depends on filesystems being mounted under /mnt/custom on the host"]
    fn test_generate_fstab() {
        let fstab = generate_fstab("/mnt/custom").unwrap();

        print!("{fstab}");

        assert!(!fstab.entries.is_empty());
        assert!(fstab.to_string().ends_with('\n'));
    }
}
