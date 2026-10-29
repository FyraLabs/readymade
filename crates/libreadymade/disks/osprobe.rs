use std::{path::PathBuf, process::Command};

use itertools::Itertools;

#[derive(Debug, Clone)]
pub struct OSProbe {
    pub part: PathBuf,
    pub os_name_pretty: String,
}

impl OSProbe {
    /// Parse a line of `os-prober` output, e.g.
    /// `/dev/sda1@/efi/Microsoft/Boot/bootmgfw.efi:Windows Boot Manager:Windows:efi`.
    ///
    /// Returns `None` if the line is malformed.
    #[tracing::instrument]
    pub fn from_entry(entry: &str) -> Option<Self> {
        // Minimum 4 parts: partition, pretty name, short name, boot type
        let mut parts = entry.split(':');
        let (Some(part), Some(os_name_pretty)) = (parts.next(), parts.next()) else {
            tracing::warn!(?entry, "Unexpected os-prober entry");
            return None;
        };
        // EFI entries have the bootloader path appended to the partition
        let part = part.split_once('@').map_or(part, |(part, _)| part);

        Some(Self {
            part: part.into(),
            os_name_pretty: os_name_pretty.to_owned(),
        })
    }

    pub fn scan() -> Option<Vec<Self>> {
        const ERROR: &str = "os-prober failed to run! Are we root? Is it installed? Continuing without OS detection.";

        let ret = tracing::info_span!("Scanning for OS").in_scope(|| {
            tracing::info!("Scanning for OS with os-prober");
            let x = Command::new("pkexec").arg("os-prober").output().ok()?;
            let strout = String::from_utf8(x.stdout).inspect_err(|e| {
                tracing::error!(?e, "os-prober should return valid utf8");
            });
            let strout = strout.ok()?;
            tracing::info!(?strout, "OS Probe Output");
            let v = strout.lines().map(str::trim);
            let v = v.filter(|l| !l.is_empty()).filter_map(Self::from_entry);
            let v = v.collect_vec();
            (!v.is_empty()).then_some(v)
        });
        ret.or_else(|| {
            tracing::error!("{ERROR}");
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_entry() {
        let os = OSProbe::from_entry(
            "/dev/nvme0n1p1@/efi/Microsoft/Boot/bootmgfw.efi:Windows Boot Manager:Windows:efi",
        )
        .unwrap();
        assert_eq!(os.part, PathBuf::from("/dev/nvme0n1p1"));
        assert_eq!(os.os_name_pretty, "Windows Boot Manager");

        let os = OSProbe::from_entry("/dev/sda2:Fedora Linux 42:Fedora:linux").unwrap();
        assert_eq!(os.part, PathBuf::from("/dev/sda2"));
        assert_eq!(os.os_name_pretty, "Fedora Linux 42");

        assert!(OSProbe::from_entry("garbage").is_none());
    }
}
