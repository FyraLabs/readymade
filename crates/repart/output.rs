use bytesize::ByteSize;
use std::path::PathBuf;

/// The output of `systemd-repart --json=…`.
///
/// See `context_dump_partitions()` in systemd's `src/repart/repart.c`.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Output {
    pub partitions: Vec<OutputPartition>,
}

impl Output {
    /// Iterate over mountpoint -> node name pairs for generating /etc/fstab
    /// from DDI partition types
    pub fn mountpoints(&self) -> impl Iterator<Item = (&'static str, String)> + '_ {
        (self.partitions.iter())
            .filter_map(|part| part.ddi_mountpoint().map(|mp| (mp, part.node.clone())))
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone)]
pub struct OutputPartition {
    /// Partition type identifier (e.g. `root-x86-64`), or the type UUID if it has no name
    #[serde(rename = "type")]
    pub part_type: String,
    /// `-` if the partition has no label
    label: String,
    uuid: uuid::Uuid,
    partno: u64,
    /// Path to repart config template
    file: PathBuf,
    /// /dev node (/dev/XXX)
    pub node: String,
    offset: u64,
    old_size: ByteSize,
    raw_size: ByteSize,
    old_padding: ByteSize,
    raw_padding: ByteSize,
    /// action systemd-repart did to this partition (`create`, `resize` or `unchanged`)
    activity: String,
    /// Only present if a partition uses Verity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    roothash: Option<String>,
    #[serde(
        default,
        rename = "drop-in_files",
        skip_serializing_if = "Option::is_none"
    )]
    drop_in_files: Option<Vec<PathBuf>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    split_path: Option<String>,
}

impl OutputPartition {
    /// The definition file this partition was created from
    #[must_use]
    pub fn file(&self) -> &std::path::Path {
        &self.file
    }

    /// Returns a Discoverable Disk Image (DDI) mountpoint if defined
    /// by checking the partition type
    ///
    /// See <https://uapi-group.org/specifications/specs/discoverable_partitions_specification/>
    #[must_use]
    pub fn ddi_mountpoint(&self) -> Option<&'static str> {
        let arch_specific = |prefix| {
            self.part_type
                .strip_prefix(prefix)
                .is_some_and(|arch| !arch.starts_with("secondary") && !arch.contains("-verity"))
        };
        match self.part_type.as_str() {
            "xbootldr" => Some("/boot"),
            "esp" => Some("/boot/efi"),
            "home" => Some("/home"),
            "srv" => Some("/srv"),
            "var" => Some("/var"),
            "tmp" => Some("/var/tmp"),
            _ if arch_specific("root-") => Some("/"),
            _ if arch_specific("usr-") => Some("/usr"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use super::*;
    const OUTPUT_EXAMPLE: &str = include_str!("test/repart-out.json");

    /// Real output of `systemd-repart --json=pretty` (systemd 259) on a disk image
    const OUTPUT_EXAMPLE_NUMERIC: &str = include_str!("test/repart-out-259.json");

    fn deserialize() -> Output {
        let output: Output = serde_json::from_str(OUTPUT_EXAMPLE).unwrap();
        println!("{output:#?}");
        output
    }

    fn partition_fixture() -> OutputPartition {
        OutputPartition {
            part_type: "root-x86-64".to_owned(),
            label: "root-x86-64".to_owned(),
            uuid: uuid::Uuid::nil(),
            partno: 1,
            file: PathBuf::from("crates/libreadymade/templates/wholedisk/50-root.conf"),
            ..Default::default()
        }
    }

    #[test]
    #[traced_test]
    fn test_ddi_mountpoint() {
        let part = partition_fixture();

        tracing::info!(?part, "Testing DDI mountpoint");

        assert_eq!(part.ddi_mountpoint(), Some("/"));

        let with_type = |t: &str| OutputPartition {
            part_type: t.to_owned(),
            ..Default::default()
        };
        assert_eq!(with_type("root-arm64").ddi_mountpoint(), Some("/"));
        assert_eq!(with_type("usr-x86-64").ddi_mountpoint(), Some("/usr"));
        assert_eq!(with_type("home").ddi_mountpoint(), Some("/home"));
        assert_eq!(with_type("root-x86-64-verity").ddi_mountpoint(), None);
        assert_eq!(with_type("root-x86-64-verity-sig").ddi_mountpoint(), None);
        assert_eq!(with_type("root-secondary").ddi_mountpoint(), None);
        assert_eq!(with_type("swap").ddi_mountpoint(), None);
        assert_eq!(with_type("linux-generic").ddi_mountpoint(), None);
    }

    #[test]
    fn test_deserialize_numeric() {
        let output: Output = serde_json::from_str(OUTPUT_EXAMPLE_NUMERIC).unwrap();
        let [esp, root] = output.partitions.as_slice() else {
            panic!("expected 2 partitions, got {output:#?}");
        };
        assert_eq!(esp.node, "img1");
        assert_eq!(esp.drop_in_files, None);
        assert_eq!(esp.raw_size, ByteSize::mib(512));
        assert_eq!(esp.old_size, ByteSize(0));
        assert_eq!(root.part_type, "root-x86-64");
        assert_eq!(root.raw_size, ByteSize(3_757_027_328));
        assert_eq!(
            root.drop_in_files.as_deref(),
            Some(&[PathBuf::from("/etc/repart.d/50-root.conf.d/override.conf")][..])
        );
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
    }
}
