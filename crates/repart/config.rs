//! systemd-repart config parser
//!
//! This module contains the types and functions for parsing and generating systemd-repart configuration files.

// Update as of 2024-09-24
// This module will be stripped of all its old validation code and everything will be serialized as-is for now.

use bytesize::ByteSize as Size;

use crate::ini_enum;

//* https://www.freedesktop.org/software/systemd/man/latest/repart.d.html

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    pub partition: Partition,
}

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
    #[serde(default = "_default_size_min_bytes", with = "size")]
    pub size_min_bytes: Size,
    #[serde(default, with = "size")]
    pub size_max_bytes: Size,
    #[serde(default, with = "size")]
    pub padding_min_bytes: Size,
    #[serde(default, with = "size")]
    pub padding_max_bytes: Size,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_blocks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<FileSystem>,
    /// `source[:target[:options]]` entries, one per `CopyFiles=` line.
    #[serde(default)]
    pub copy_files: Vec<String>,

    // Btrfs-exclusive options
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_level: Option<String>,

    // These are whitespace-separated and may be specified multiple times
    #[serde(default, deserialize_with = "whitespace_separated")]
    pub exclude_files: Vec<String>,
    #[serde(default, deserialize_with = "whitespace_separated")]
    pub exclude_files_target: Vec<String>,
    #[serde(default, deserialize_with = "whitespace_separated")]
    pub make_directories: Vec<String>,
    #[serde(default, deserialize_with = "whitespace_separated")]
    pub subvolumes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_subvolume: Option<String>,
    #[serde(default)]
    pub encrypt: EncryptOption,
    #[serde(default)]
    pub verity: Verity,

    #[serde(default, deserialize_with = "systemd_bool")]
    pub factory_reset: bool,

    // Takes at least one and at most two fields separated with a colon (":").
    // #[serde_as(as = "(PathBuf, StringWithSeparator::<SpaceSeparator, String>)")]
    // #[serde_as(as = "StringWithSeparator::<ColonSeparator, (PathBuf, StringWithSeparator::<CommaSeperator, String)>")]
    // mount_point: (PathBuf, Option<String>),
    #[serde(default)]
    pub mount_point: Vec<String>,
}

impl Partition {
    #[must_use]
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

/// A systemd boolean, as parsed by `parse_boolean()`, or a native boolean.
fn systemd_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;

    impl serde::de::Visitor<'_> for Visitor {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a systemd boolean")
        }

        fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<bool, E> {
            Ok(v)
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<bool, E> {
            match v.to_ascii_lowercase().as_str() {
                "1" | "yes" | "y" | "true" | "t" | "on" => Ok(true),
                "0" | "no" | "n" | "false" | "f" | "off" => Ok(false),
                _ => Err(E::invalid_value(serde::de::Unexpected::Str(v), &self)),
            }
        }
    }

    deserializer.deserialize_any(Visitor)
}

/// A list of whitespace-separated values, which may also be split over multiple keys.
fn whitespace_separated<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a whitespace-separated list")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.split_whitespace().map(ToOwned::to_owned).collect())
        }

        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = vec![];
            while let Some(s) = seq.next_element::<String>()? {
                out.extend(s.split_whitespace().map(ToOwned::to_owned));
            }
            Ok(out)
        }
    }

    deserializer.deserialize_any(Visitor)
}

/// Sizes in the format systemd's `parse_size()` uses, where suffixes are powers of 1024.
///
/// [`bytesize`]'s own parser treats e.g. `16M` as 16 megabytes instead of 16 mebibytes.
mod size {
    use bytesize::ByteSize;

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "signature required by serde"
    )]
    pub fn serialize<S: serde::Serializer>(size: &ByteSize, se: S) -> Result<S::Ok, S::Error> {
        se.serialize_u64(size.as_u64())
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(de: D) -> Result<ByteSize, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = ByteSize;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a size in bytes, optionally with a K, M, G, T, P or E suffix")
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<ByteSize, E> {
                Ok(ByteSize(v))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ByteSize, E> {
                parse(v)
                    .map(ByteSize)
                    .ok_or_else(|| E::invalid_value(serde::de::Unexpected::Str(v), &self))
            }
        }

        de.deserialize_any(Visitor)
    }

    /// Parse a size like systemd's `parse_size()` with a base of 1024, e.g. `1G 512M` or `1.5K`.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn parse(s: &str) -> Option<u64> {
        const SUFFIXES: [(char, u32); 7] = [
            ('B', 0),
            ('K', 1),
            ('M', 2),
            ('G', 3),
            ('T', 4),
            ('P', 5),
            ('E', 6),
        ];
        let mut rest = s.trim();
        if rest.is_empty() {
            return None;
        }
        let mut total = 0u64;
        while !rest.is_empty() {
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(rest.len());
            let (number, tail) = rest.split_at(end);
            let number = number.parse::<f64>().ok()?;
            let tail = tail.trim_start();
            let (factor, tail) = match tail.chars().next() {
                Some(c) => match SUFFIXES.iter().find(|(s, _)| *s == c) {
                    Some(&(_, exp)) => (1024u64.pow(exp), &tail[1..]),
                    None => return None,
                },
                None => (1, tail),
            };
            let bytes = number * factor as f64;
            if !bytes.is_finite() || bytes >= u64::MAX as f64 {
                return None;
            }
            total = total.checked_add(bytes as u64)?;
            rest = tail.trim_start();
        }
        Some(total)
    }
}

const fn _default_weight() -> u32 {
    1000
}

const fn _default_size_min_bytes() -> Size {
    Size::mib(10)
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

/// The architecture identifier systemd uses for the current target, as in `root-{arch}`.
///
/// See `architecture_table` in systemd's `src/basic/architecture.c`.
#[must_use]
pub const fn systemd_arch() -> &'static str {
    let little = cfg!(target_endian = "little");
    match std::env::consts::ARCH.as_bytes() {
        b"x86_64" => "x86-64",
        b"x86" => "x86",
        b"aarch64" if little => "arm64",
        b"aarch64" => "arm64-be",
        b"arm" if little => "arm",
        b"arm" => "arm-be",
        b"loongarch64" => "loongarch64",
        b"riscv32" => "riscv32",
        b"riscv64" => "riscv64",
        b"powerpc" if little => "ppc-le",
        b"powerpc" => "ppc",
        b"powerpc64" if little => "ppc64-le",
        b"powerpc64" => "ppc64",
        b"s390x" => "s390x",
        b"mips" if little => "mips-le",
        b"mips" => "mips",
        b"mips64" if little => "mips64-le",
        b"mips64" => "mips64",
        b"sparc64" => "sparc64",
        b"m68k" => "m68k",
        _ => std::env::consts::ARCH,
    }
}

impl serde::Serialize for PartTypeIdent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let arch = systemd_arch();
        let arch_type;
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
            Self::RootArch => {
                arch_type = format!("root-{arch}");
                &arch_type
            }
            Self::RootArchVerity => {
                arch_type = format!("root-{arch}-verity");
                &arch_type
            }
            Self::RootArchVeritySig => {
                arch_type = format!("root-{arch}-verity-sig");
                &arch_type
            }
            Self::Usr => "usr",
            Self::UsrVerity => "usr-verity",
            Self::UsrVeritySig => "usr-verity-sig",
            Self::UsrSecondary => "usr-secondary",
            Self::UsrSecondaryVerity => "usr-secondary-verity",
            Self::UsrSecondaryVeritySig => "usr-secondary-verity-sig",
            Self::UsrArch => {
                arch_type = format!("usr-{arch}");
                &arch_type
            }
            Self::UsrArchVerity => {
                arch_type = format!("usr-{arch}-verity");
                &arch_type
            }
            Self::UsrArchVeritySig => {
                arch_type = format!("usr-{arch}-verity-sig");
                &arch_type
            }
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
        let arch = systemd_arch();
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
            s if s == format!("root-{arch}") => Ok(Self::RootArch),
            s if s == format!("root-{arch}-verity") => Ok(Self::RootArchVerity),
            s if s == format!("root-{arch}-verity-sig") => Ok(Self::RootArchVeritySig),
            "usr" => Ok(Self::Usr),
            "usr-verity" => Ok(Self::UsrVerity),
            "usr-verity-sig" => Ok(Self::UsrVeritySig),
            "usr-secondary" => Ok(Self::UsrSecondary),
            "usr-secondary-verity" => Ok(Self::UsrSecondaryVerity),
            "usr-secondary-verity-sig" => Ok(Self::UsrSecondaryVeritySig),
            s if s == format!("usr-{arch}") => Ok(Self::UsrArch),
            s if s == format!("usr-{arch}-verity") => Ok(Self::UsrArchVerity),
            s if s == format!("usr-{arch}-verity-sig") => Ok(Self::UsrArchVeritySig),
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
    use super::*;

    #[test]
    fn read_config() {
        let config = include_str!("test/submarine.conf");
        let res: Config = serde_systemd_unit::from_str(config).unwrap();
        assert_eq!(res.partition.size_min_bytes, Size::mib(16));
        assert_eq!(res.partition.size_max_bytes, Size::mib(16));
        assert!(res.partition.factory_reset);
        assert!(matches!(res.partition.part_type, PartTypeIdent::Unknown(_)));

        let config2 = include_str!("test/root.conf");
        let res2: Config = serde_systemd_unit::from_str(config2).unwrap();
        let p = &res2.partition;
        assert!(matches!(p.part_type, PartTypeIdent::Root));
        assert!(matches!(p.format, Some(FileSystem::Btrfs)));
        assert!(matches!(p.encrypt, EncryptOption::KeyFile));
        assert_eq!(p.copy_files, ["/:/"]);
        assert_eq!(p.exclude_files, ["/boot/"]);
        assert_eq!(
            p.mount_point_as_tuple(),
            [
                (
                    "/".to_owned(),
                    Some("rw,seclabel,relatime,discard=async".to_owned())
                ),
                (
                    "/home".to_owned(),
                    Some("rw,seclabel,relatime,discard=async".to_owned())
                ),
            ]
        );
    }

    #[test]
    fn read_readymade_templates() {
        for (i, config) in [
            include_str!("../libreadymade/templates/wholedisk/20-efi.conf"),
            include_str!("../libreadymade/templates/wholedisk/45-boot.conf"),
            include_str!("../libreadymade/templates/wholedisk/50-root.conf"),
            include_str!("../libreadymade/templates/bootcwholedisk/20-efi.conf"),
            include_str!("../libreadymade/templates/bootcwholedisk/45-boot.conf"),
            include_str!("../libreadymade/templates/bootcwholedisk/50-root.conf"),
            include_str!("../libreadymade/templates/chromebookinstall/20-efi.conf"),
            include_str!("../libreadymade/templates/chromebookinstall/40-submarine.conf"),
            include_str!("../libreadymade/templates/chromebookinstall/45-boot.conf"),
            include_str!("../libreadymade/templates/chromebookinstall/50-root.conf"),
        ]
        .into_iter()
        .enumerate()
        {
            let config: Config = serde_systemd_unit::from_str(config)
                .unwrap_or_else(|e| panic!("template #{i} failed to parse: {e}"));
            assert!(!matches!(
                config.partition.part_type,
                PartTypeIdent::LinuxGeneric
            ));
        }
    }

    #[test]
    fn multi_value_settings() {
        let config: Config = serde_systemd_unit::from_str(
            "[Partition]
Type=root
CopyFiles=/a:/b
CopyFiles=/c:/d:fsverity=copy
ExcludeFiles=/x /y
ExcludeFiles=/z
Subvolumes=/ /home
FactoryReset=on
",
        )
        .unwrap();
        let p = config.partition;
        assert_eq!(p.copy_files, ["/a:/b", "/c:/d:fsverity=copy"]);
        assert_eq!(p.exclude_files, ["/x", "/y", "/z"]);
        assert_eq!(p.subvolumes, ["/", "/home"]);
        assert!(p.factory_reset);
    }

    #[test]
    fn sizes_are_base_1024() {
        assert_eq!(size::parse("512"), Some(512));
        assert_eq!(size::parse("512B"), Some(512));
        assert_eq!(size::parse("16M"), Some(16 << 20));
        assert_eq!(size::parse("1G"), Some(1 << 30));
        assert_eq!(size::parse("1.5K"), Some(1536));
        assert_eq!(size::parse("1G 512M"), Some((1 << 30) + (512 << 20)));
        assert_eq!(size::parse("2T"), Some(2 << 40));
        assert_eq!(size::parse(""), None);
        assert_eq!(size::parse("16MB"), None);
        assert_eq!(size::parse("lots"), None);
        assert_eq!(size::parse("99999999E"), None);
    }

    #[test]
    fn part_type_arch() {
        let arch = systemd_arch();
        assert!(!arch.contains('_'), "{arch}");
        if cfg!(target_arch = "x86_64") {
            assert_eq!(arch, "x86-64");
        }
        if cfg!(all(target_arch = "aarch64", target_endian = "little")) {
            assert_eq!(arch, "arm64");
        }
        let root: PartTypeIdent = serde_systemd_unit::from_str::<Config>(&format!(
            "[Partition]\nType=root-{arch}-verity"
        ))
        .unwrap()
        .partition
        .part_type;
        assert!(matches!(root, PartTypeIdent::RootArchVerity));
        assert_eq!(
            serde_json::to_string(&PartTypeIdent::UsrArch).unwrap(),
            format!("\"usr-{arch}\"")
        );
    }

    #[test]
    fn unknown_enum_variant() {
        let err = serde_systemd_unit::from_str::<Config>("[Partition]\nType=root\nEncrypt=yes")
            .unwrap_err()
            .to_string();
        assert!(err.contains("key-file+tpm2"), "{err}");
    }

    #[test]
    fn serialize_round_trip() {
        let config = Config {
            partition: Partition {
                part_type: PartTypeIdent::Esp,
                label: Some("My Label".to_owned()),
                uuid: Some(uuid::uuid!("7466c448-87ac-4e1c-b3e3-fe83b7a19262")),
                size_min_bytes: Size::mib(512),
                size_max_bytes: Size::gib(1),
                copy_blocks: Some("/dev/sda1".to_owned()),
                format: Some(FileSystem::Vfat),
                copy_files: vec!["/boot/efi:/".to_owned()],
                exclude_files: vec!["/a".to_owned(), "/b".to_owned()],
                encrypt: EncryptOption::KeyFileTpm2,
                factory_reset: true,
                mount_point: vec!["/boot/efi:umask=0077".to_owned()],
                ..Default::default()
            },
        };
        let s = serde_systemd_unit::to_string(&config).unwrap();
        assert!(
            s.starts_with("[Partition]\nType=esp\nLabel=My Label\n"),
            "{s}"
        );
        assert!(s.contains("\nSizeMinBytes=536870912\n"), "{s}");
        assert!(s.contains("\nEncrypt=key-file+tpm2\n"), "{s}");
        assert!(s.contains("\nExcludeFiles=/a\nExcludeFiles=/b\n"), "{s}");

        let parsed: Config = serde_systemd_unit::from_str(&s).unwrap();
        let json = |c: &Config| serde_json::to_value(c).unwrap();
        assert_eq!(json(&parsed), json(&config));
        // and through JSON, which is how configs are passed around
        let from_json: Config = serde_json::from_value(json(&config)).unwrap();
        assert_eq!(json(&from_json), json(&config));
    }
}
