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
use std::env::consts::ARCH;

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
        let value = u64::deserialize(deserializer)?;
        Ok(Self {
            inner: ByteSize::b(value),
        })
    }
}

//* https://www.freedesktop.org/software/systemd/man/latest/repart.d.html

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct RepartConfig {
    partition: Partition,
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Partition {
    r#type: PartTypeIdent,
    label: String,
    #[serde(rename = "UUID")]
    uuid: uuid::Uuid,
    priority: i32,
    // #[validate(range(min = 0, max = 1_000_000))]
    #[serde(default = "_default_weight")]
    weight: u32,
    // #[validate(range(min = 0, max = 1_000_000))]
    #[serde(default)]
    padding_weight: u32,
    #[serde(default = "_default_size_min_bytes")]
    size_min_bytes: Size,
    #[serde(default)]
    size_max_bytes: Size,
    #[serde(default)]
    padding_min_bytes: Size,
    #[serde(default)]
    padding_max_bytes: Size,
    #[serde(skip_serializing_if = "Option::is_none")]
    copy_blocks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<FileSystem>,
    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<ColonSeparator, String>")]
    copy_files: Vec<String>,

    // todo: serialize ; and whitespace-separated values as vec

    // separate by ;
    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<SemicolonSeparator, String>")]
    exclude_files: Vec<String>,
    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<SemicolonSeparator, String>")]
    exclude_files_target: Vec<String>,

    // separate by whitespace
    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<SpaceSeparator, String>")]
    make_directories: Vec<String>,

    #[serde(default)]
    #[serde_as(as = "StringWithSeparator::<SpaceSeparator, String>")]
    subvolumes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_subvolume: Option<String>,
    #[serde(default)]
    encrypt: EncryptOption,
    #[serde(default)]
    verity: Verity,

    #[serde(default)]
    #[serde(serialize_with = "turn_to_string")]
    factory_reset: bool,

    // Takes at least one and at most two fields separated with a colon (":").
    // #[serde_as(as = "(PathBuf, StringWithSeparator::<SpaceSeparator, String>)")]
    // #[serde_as(as = "StringWithSeparator::<ColonSeparator, (PathBuf, StringWithSeparator::<CommaSeperator, String)>")]
    // mount_point: (PathBuf, Option<String>),
    #[serde(default)]
    mount_point: String,
}

fn turn_to_string<T, S>(value: &T, se: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    T: std::fmt::Display,
{
    se.serialize_str(&format!("{value}"))
}

const fn _default_weight() -> u32 {
    1000
}

const fn _default_size_min_bytes() -> Size {
    Size {
        inner: ByteSize::mib(10),
    }
}

#[derive(Debug, Default, serde::Deserialize)]
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
#[inline(always)]
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


ini_enum! {
    #[derive(Debug, serde::Deserialize)]
    pub enum FileSystem {
        Ext4,
        Btrfs,
        Xfs,
        Vfat,
        Erofs,
        Squashfs,
        Swap,
    }

    #[derive(Debug, Deserialize, Default)]
    pub enum EncryptOption {
        #[default]
        Off,
        KeyFile,
        Tpm2,
        KeyFileTpm2 => "key-file+tpm2",
    }

    #[derive(Debug, Default, Deserialize)]
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

    use super::{Partition, RepartConfig};

    #[test]
    fn read_config() {
        let config = include_str!("test/submarine.conf");
        let res: RepartConfig = serde_ini::from_str(config).unwrap();

        println!("{res:#?}");
    }

    #[test]
    fn ser_new_config() {
        let mount_point = "idk".to_owned();
        let res = serde_ini::to_string(&RepartConfig {
            partition: Partition {
                r#type: super::PartTypeIdent::Esp,
                label: "My Label".to_owned(),
                uuid: uuid::uuid!("7466c448-87ac-4e1c-b3e3-fe83b7a19262"),
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
                mount_point,
            },
        })
        .unwrap();
        println!("{res}");
    }
}
