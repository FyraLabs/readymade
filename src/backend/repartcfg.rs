//! systemd-repart config parser
//!
//! This module contains the types and functions for parsing and generating systemd-repart configuration files.

// Update as of 2024-09-24
// This module will be stripped of all its old validation code and everything will be serialized as-is for now.

use bytesize::ByteSize;
use serde::Deserialize;
use serde_with::{
    formats::{ColonSeparator, SemicolonSeparator, SpaceSeparator},
    serde_as, StringWithSeparator,
};
use std::{env::consts::ARCH, str::FromStr};

use crate::ini_enum;

#[derive(Debug, Copy, Clone, Default)]
pub struct Size {
    pub inner: ByteSize,
}

impl From<ByteSize> for Size {
    fn from(value: ByteSize) -> Self {
        Self { inner: value }
    }
}

impl serde::Serialize for Size {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.inner.as_u64())
    }
}

impl<'de> serde::Deserialize<'de> for Size {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // This data will usually be a string, so we need to parse it into a ByteSize
        let size = ByteSize::from_str(&String::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)?;
        Ok(Self { inner: size })
    }
}

//* https://www.freedesktop.org/software/systemd/man/latest/repart.d.html

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct RepartConfig {
    pub partition: Partition,
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Partition {
    #[serde(rename = "Type")]
    pub part_type: PartTypeIdent,
    pub label: Option<String>,
    #[serde(rename = "UUID")]
    pub uuid: Option<uuid::Uuid>,
    #[serde(default)]
    pub priority: i32,
    // #[validate(range(min = 0, max = 1_000_000))]
    #[serde(default = "_default_weight")]
    pub weight: u32,
    // #[validate(range(min = 0, max = 1_000_000))]
    #[serde(default)]
    pub padding_weight: u32,
    #[serde(default = "_default_size_min_bytes")]
    pub size_min_bytes: Size,
    #[serde(default)]
    pub size_max_bytes: Size,
    #[serde(default)]
    pub padding_min_bytes: Size,
    #[serde(default)]
    pub padding_max_bytes: Size,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_blocks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<FileSystem>,
    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<ColonSeparator, String>")]
    pub copy_files: Vec<String>,

    // Btrfs-exclusive options
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_level: Option<String>,

    // todo: serialize ; and whitespace-separated values as vec

    // separate by ;
    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<SemicolonSeparator, String>")]
    pub exclude_files: Vec<String>,
    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<SemicolonSeparator, String>")]
    pub exclude_files_target: Vec<String>,

    // separate by whitespace
    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<SpaceSeparator, String>")]
    pub make_directories: Vec<String>,

    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<SpaceSeparator, String>")]
    pub subvolumes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_subvolume: Option<String>,
    #[serde(default)]
    pub encrypt: EncryptOption,
    #[serde(default)]
    pub verity: Verity,

    #[serde(default)]
    #[serde(serialize_with = "turn_to_string")]
    #[serde(deserialize_with = "bool_from_string")]
    pub factory_reset: bool,

    // Takes at least one and at most two fields separated with a colon (":").
    // #[serde_as(as = "(PathBuf, StringWithSeparator::<SpaceSeparator, String>)")]
    // #[serde_as(as = "StringWithSeparator::<ColonSeparator, (PathBuf, StringWithSeparator::<CommaSeperator, String)>")]
    // mount_point: (PathBuf, Option<String>),
    #[serde(default)]
    pub mount_point: Vec<String>,
}

impl Partition {
    pub fn mount_point_as_tuple(&self) -> Vec<(String, Option<String>)> {
        self.mount_point
            .iter()
            .filter_map(|mount_point| {
                if mount_point.is_empty() {
                    return None;
                }
                // If there's a colon, split it into two fields
                // only the first colon is considered though, so if there are more than one, the rest are ignored
                let mut parts = mount_point.splitn(2, ':');
                let fst = parts.next()?.to_owned();
                let snd = parts.next().map(std::borrow::ToOwned::to_owned);
                Some((fst, snd))
            })
            .collect()
    }
}

fn turn_to_string<T, S>(value: &T, se: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    T: std::fmt::Display,
{
    se.serialize_str(&format!("{value}"))
}

// Convert a systemd boolean value into a boolean
// This means that "yes" and "true" are true, and "no" and "false" are false
// Same goes for "1" and "0"
fn bool_from_string<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "yes" | "true" | "1" => Ok(true),
        "no" | "false" | "0" => Ok(false),
        _ => Err(serde::de::Error::custom("invalid boolean value")),
    }
}

const fn _default_weight() -> u32 {
    1000
}

const fn _default_size_min_bytes() -> Size {
    Size {
        inner: ByteSize::mib(10),
    }
}

#[derive(Debug, Default)]
#[must_use]
pub enum PartTypeIdent {
    Esp,
    Xbootldr,
    Swap,
    Home,
    Srv,
    Var,
    Tmp,
    #[default]
    LinuxGeneric,
    Root,
    RootVerity,
    RootVeritySig,
    RootSecondary,
    RootSecondaryVerity,
    RootSecondaryVeritySig,
    RootArch,
    RootArchVerity,
    RootArchVeritySig,
    Usr,
    UsrVerity,
    UsrVeritySig,
    UsrSecondary,
    UsrSecondaryVerity,
    UsrSecondaryVeritySig,
    UsrArch,
    UsrArchVerity,
    UsrArchVeritySig,
    Unknown(String),
}

// todo: somehow convert these into hyphenated strings
const ROOT_ARCH: &str = const_format::formatcp!("root-{ARCH}");
const ROOT_ARCH_VERITY: &str = const_format::formatcp!("root-{ARCH}-verity");
const ROOT_ARCH_VERITY_SIG: &str = const_format::formatcp!("root-{ARCH}-verity-sig");
const USR_ARCH: &str = const_format::formatcp!("usr-{ARCH}");
const USR_ARCH_VERITY: &str = const_format::formatcp!("usr-{ARCH}-verity");
const USR_ARCH_VERITY_SIG: &str = const_format::formatcp!("usr-{ARCH}-verity-sig");

/// Converts a &str into a new &str with hyphens instead of underscores
#[must_use]
#[inline]
fn underscore_to_hyphen(input: &str) -> String {
    input.replace('_', "-")
}

impl serde::Serialize for PartTypeIdent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let root_arch = underscore_to_hyphen(ROOT_ARCH);
        let root_arch_verity = underscore_to_hyphen(ROOT_ARCH_VERITY);
        let root_arch_verity_sig = underscore_to_hyphen(ROOT_ARCH_VERITY_SIG);
        let usr_arch = underscore_to_hyphen(USR_ARCH);
        let usr_arch_verity = underscore_to_hyphen(USR_ARCH_VERITY);
        let usr_arch_verity_sig = underscore_to_hyphen(USR_ARCH_VERITY_SIG);
        serializer.serialize_str(match self {
            Self::Esp => "esp",
            Self::Xbootldr => "xbootldr",
            Self::Swap => "swap",
            Self::Home => "home",
            Self::Srv => "srv",
            Self::Var => "var",
            Self::Tmp => "tmp",
            Self::LinuxGeneric => "linux-generic",
            Self::Root => "root",
            Self::RootVerity => "root-verity",
            Self::RootVeritySig => "root-verity-sig",
            Self::RootSecondary => "root-secondary",
            Self::RootSecondaryVerity => "root-secondary-verity",
            Self::RootSecondaryVeritySig => "root-secondary-verity-sig",
            Self::RootArch => &root_arch,
            Self::RootArchVerity => &root_arch_verity,
            Self::RootArchVeritySig => &root_arch_verity_sig,
            Self::Usr => "usr",
            Self::UsrVerity => "usr-verity",
            Self::UsrVeritySig => "usr-verity-sig",
            Self::UsrSecondary => "usr-secondary",
            Self::UsrSecondaryVerity => "usr-secondary-verity",
            Self::UsrSecondaryVeritySig => "usr-secondary-verity-sig",
            Self::UsrArch => &usr_arch,
            Self::UsrArchVerity => &usr_arch_verity,
            Self::UsrArchVeritySig => &usr_arch_verity_sig,
            Self::Unknown(x) => x,
        })
    }
}

impl<'de> serde::Deserialize<'de> for PartTypeIdent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "esp" => Ok(Self::Esp),
            "xbootldr" => Ok(Self::Xbootldr),
            "swap" => Ok(Self::Swap),
            "home" => Ok(Self::Home),
            "srv" => Ok(Self::Srv),
            "var" => Ok(Self::Var),
            "tmp" => Ok(Self::Tmp),
            "linux-generic" => Ok(Self::LinuxGeneric),
            "root" => Ok(Self::Root),
            "root-verity" => Ok(Self::RootVerity),
            "root-verity-sig" => Ok(Self::RootVeritySig),
            "root-secondary" => Ok(Self::RootSecondary),
            "root-secondary-verity" => Ok(Self::RootSecondaryVerity),
            "root-secondary-verity-sig" => Ok(Self::RootSecondaryVeritySig),
            s if s == underscore_to_hyphen(ROOT_ARCH) => Ok(Self::RootArch),
            s if s == underscore_to_hyphen(ROOT_ARCH_VERITY) => Ok(Self::RootArchVerity),
            s if s == underscore_to_hyphen(ROOT_ARCH_VERITY_SIG) => Ok(Self::RootArchVeritySig),
            "usr" => Ok(Self::Usr),
            "usr-verity" => Ok(Self::UsrVerity),
            "usr-verity-sig" => Ok(Self::UsrVeritySig),
            "usr-secondary" => Ok(Self::UsrSecondary),
            "usr-secondary-verity" => Ok(Self::UsrSecondaryVerity),
            "usr-secondary-verity-sig" => Ok(Self::UsrSecondaryVeritySig),
            s if s == underscore_to_hyphen(USR_ARCH) => Ok(Self::UsrArch),
            s if s == underscore_to_hyphen(USR_ARCH_VERITY) => Ok(Self::UsrArchVerity),
            s if s == underscore_to_hyphen(USR_ARCH_VERITY_SIG) => Ok(Self::UsrArchVeritySig),
            _ => Ok(Self::Unknown(s.clone())),
        }
    }
}

ini_enum! {
    #[derive(Debug, Clone, Copy)]
    // #[serde(rename_all = "lowercase")]
    pub enum FileSystem {
        Ext4,
        Btrfs,
        Xfs,
        Vfat,
        Erofs,
        Squashfs,
        Swap,
    }

    #[derive(Debug, Default, Clone, Copy)]
    // #[serde(rename_all = "lowercase")]
    pub enum EncryptOption {
        #[default]
        Off,
        KeyFile => "key-file",
        Tpm2,
        KeyFileTpm2 => "key-file+tpm2",
    }

    #[derive(Debug, Default, Clone, Copy)]
    // #[serde(rename_all = "lowercase")]
    pub enum Verity {
        #[default]
        Off,
        Data,
        Hash,
        Signature,
    }
}

#[cfg(test)]
mod tests {
    use super::RepartConfig;

    #[test]
    fn read_config() {
        let config = include_str!("test/submarine.conf");
        let res: RepartConfig = serde_systemd_unit::from_str(config).unwrap();

        println!("{res:#?}");
        println!("{:?}", res.partition.mount_point_as_tuple());

        let config2 = include_str!("test/root.conf");
        let res2: RepartConfig = serde_systemd_unit::from_str(config2).unwrap();

        println!("{res2:#?}");
        println!("{:?}", res2.partition.mount_point_as_tuple());
    }

    // FIXME: port this to serde_systemd_unit
    /*
    #[test]
    fn ser_new_config() {
        let res = serde_ini::to_string(&RepartConfig {
            partition: Partition {
                part_type: super::PartTypeIdent::Esp,
                label: Some("My Label".to_owned()),
                uuid: Some(uuid::uuid!("7466c448-87ac-4e1c-b3e3-fe83b7a19262")),
                priority: Default::default(),
                weight: Default::default(),
                padding_weight: Default::default(),
                size_min_bytes: super::_default_size_min_bytes(),
                size_max_bytes: super::Size {
                    inner: bytesize::ByteSize::kb(100),
                },
                padding_min_bytes: super::Size::default(),
                padding_max_bytes: super::Size::default(),
                copy_blocks: Some("hai".to_owned()),
                format: Some(super::FileSystem::Ext4),
                copy_files: vec![],
                exclude_files: vec![],
                exclude_files_target: vec![],
                make_directories: vec![],
                subvolumes: vec![],
                default_subvolume: None,
                encrypt: crate::backend::repartcfg::EncryptOption::default(),
                verity: super::Verity::Off,
                factory_reset: false,
                mount_point: vec![],
                ..Default::default()
            },
        })
        .unwrap();
        println!("{res}");
    }*/
}
