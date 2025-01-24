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
}

type Result<T> = std::result::Result<T, FsTableError>;

/// The order in which the filesystems should be checked.
#[derive(Debug, Clone, Default)]
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

#[derive(Debug, Clone, Default)]
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
    /// ...
    ///
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

impl FsEntry {
    /// Parse a FsEntry from a line in the fstab file.
    ///

    pub fn from_line_str(line: &str) -> Result<Self> {
        // split by whitespace
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 6 {
            return Err(FsTableError::InvalidEntry(line.to_string()));
        }

        let device_spec = parts[0].to_string();

        let mountpoint = if parts[1] == "none" {
            None
        } else {
            Some(parts[1].to_string())
        };

        let fs_type = parts[2].to_string();

        let options = parts[3].split(',').map(|s| s.to_string()).collect();

        let dump_freq = parts[4]
            .parse::<u8>()
            .map_err(|_| FsTableError::InvalidEntry(line.to_string()))?;
        let pass = FsckOrder::try_from(parts[5])?;

        Ok(Self {
            device_spec,
            mountpoint,
            fs_type,
            options,
            dump_freq,
            pass,
        })
    }

    /// Serialize the FsEntry into a string that can be written to the fstab file.
    pub fn to_line_str(&self) -> String {
        let mountpoint = self.mountpoint.as_deref().unwrap_or("none");
        let options = if self.options.is_empty() {
            "defaults".to_string()
        } else {
            self.options.join(",")
        };
        let pass = self.pass.clone() as u8;

        format!(
            "{device_spec}\t{mountpoint}\t{fs_type}\t{options}\t{dump_freq}\t{pass}",
            device_spec = self.device_spec,
            mountpoint = mountpoint,
            fs_type = self.fs_type,
            options = options,
            pass = pass,
            dump_freq = self.dump_freq,
        )
    }
}

impl TryFrom<&str> for FsEntry {
    type Error = FsTableError;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_line_str(value)
    }
}

impl ToString for FsEntry {
    fn to_string(&self) -> String {
        self.to_line_str()
    }
}

#[derive(Debug)]
pub struct FsTable {
    pub entries: Vec<FsEntry>,
}

impl FsTable {
    pub fn from_str(table: &str) -> Result<Self> {
        let entries = table
            .lines()
            .map(|line| FsEntry::from_line_str(line))
            .collect::<Result<Vec<FsEntry>>>()?;

        Ok(Self { entries })
    }

    pub fn to_string(&self) -> String {
        self.entries
            .iter()
            .map(|entry| entry.to_line_str())
            .collect::<Vec<String>>()
            .join("\n")
    }
}

impl TryFrom<&str> for FsTable {
    type Error = FsTableError;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_str(value)
    }
}

pub fn read_mtab() -> Result<FsTable> {
    let mtab = std::fs::read_to_string("/etc/mtab")
        .map_err(|e| FsTableError::InvalidEntry(e.to_string()))?;
    FsTable::from_str(&mtab)
}

pub fn read_fstab() -> Result<FsTable> {
    let fstab = std::fs::read_to_string("/etc/fstab")
        .map_err(|e| FsTableError::InvalidEntry(e.to_string()))?;
    FsTable::from_str(&fstab)
}

/// Generate a new fstab from mtab, using a chroot prefix to generate the new fstab.
pub fn generate_fstab(prefix: &str) -> Result<FsTable> {
    let mtab = read_mtab()?;

    // filter by prefix
    let entries = mtab
        .entries
        .iter()
        .filter_map(|entry| {
            entry.mountpoint.as_ref().and_then(|mp| {
                if mp.starts_with(prefix) {
                    let mut new_entry = entry.clone();
                    new_entry.mountpoint = Some(
                        match mp.strip_prefix(prefix).unwrap() {
                            "" => "/",
                            path => path,
                        }
                        .to_string(),
                    );
                    Some(new_entry)
                } else {
                    None
                }
            })
        })
        .collect();

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
        assert_eq!(entry.mountpoint, Some("/".to_string()));
        assert_eq!(entry.fs_type, "ext4");
        assert_eq!(entry.options, vec!["rw", "relatime"]);
        assert_eq!(entry.dump_freq, 0);
        assert_eq!(entry.pass as u8, 1);
    }

    #[test]
    fn test_fstab_serialize() {
        let entry = FsEntry {
            device_spec: "/dev/sda1".to_string(),
            mountpoint: Some("/".to_string()),
            fs_type: "ext4".to_string(),
            options: vec!["rw".to_string(), "relatime".to_string()],
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
        assert!(FsckOrder::try_from(&3u8).is_err());
    }

    #[test]
    fn test_fstab_table() {
        let table = "/dev/sda1\t/\text4\trw,relatime\t0\t1\n/dev/sda2\tnone\tswap\tsw\t0\t0";
        let fstab = FsTable::from_str(table).unwrap();

        assert_eq!(fstab.entries.len(), 2);
        assert_eq!(fstab.entries[0].device_spec, "/dev/sda1");
        assert_eq!(fstab.entries[1].device_spec, "/dev/sda2");

        let serialized = fstab.to_string();
        assert_eq!(serialized, table);
    }

    #[test]
    fn test_mtab_parse() {
        let mtab = std::fs::read_to_string("/etc/mtab").unwrap();

        let table = FsTable::from_str(&mtab).unwrap();

        println!("{:#?}", table.to_string());
    }
}
