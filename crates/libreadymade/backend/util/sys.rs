use std::path::Path;

/// Check if the current running system is UEFI or not.
///
/// Simply checks for the existence of the `/sys/firmware/efi` directory.
///
/// False negatives are possible if the system is booted in BIOS mode and the UEFI variables are not exposed.
#[must_use]
pub fn check_uefi() -> bool {
    std::fs::metadata("/sys/firmware/efi").is_ok()
}

const PCI_VENDOR_NVIDIA: &str = "0x10de";
/// PCI base class for display controllers (VGA, 3D, ...).
const PCI_CLASS_DISPLAY: &str = "0x03";
const EFIVAR_SECURE_BOOT: &str =
    "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-e098032b8c00";

/// Check whether an NVIDIA display controller is present on the PCI bus.
#[must_use]
pub fn has_nvidia_display() -> bool {
    has_nvidia_display_in(Path::new("/sys/bus/pci/devices"))
}

fn has_nvidia_display_in(pci_devices: &Path) -> bool {
    let read = |dev: &Path, attr: &str| {
        std::fs::read_to_string(dev.join(attr))
            .map(|s| s.trim().to_ascii_lowercase())
            .unwrap_or_default()
    };
    std::fs::read_dir(pci_devices)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .any(|dev| {
            read(&dev, "vendor") == PCI_VENDOR_NVIDIA
                && read(&dev, "class").starts_with(PCI_CLASS_DISPLAY)
        })
}

/// Check whether the system was booted with UEFI Secure Boot enabled.
#[must_use]
pub fn secure_boot_enabled() -> bool {
    secure_boot_enabled_in(Path::new(EFIVAR_SECURE_BOOT))
}

fn secure_boot_enabled_in(efivar: &Path) -> bool {
    // efivarfs files start with a 4-byte attribute header, followed by the variable data.
    std::fs::read(efivar).is_ok_and(|data| data.get(4) == Some(&1))
}

/// The filesystem type mounted at `target` in a `/proc/mounts`-style table. Later entries
/// shadow earlier ones, like stacked mounts do.
#[must_use]
pub fn mount_fstype<'a>(mounts: &'a str, target: &str) -> Option<&'a str> {
    mounts
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(' ').skip(1);
            Some((fields.next()?, fields.next()?))
        })
        .rfind(|&(mountpoint, _)| mountpoint == target)
        .map(|(_, fstype)| fstype)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS: &str = "\
/dev/vda3 / btrfs rw,relatime,compress=zstd:1,subvol=/ 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
/dev/vda3 /var btrfs rw,relatime,compress=zstd:1,subvol=/var 0 0
/dev/vda2 /boot ext4 rw,relatime 0 0
";

    #[test]
    fn finds_mount_fstype() {
        assert_eq!(mount_fstype(MOUNTS, "/"), Some("btrfs"));
        assert_eq!(mount_fstype(MOUNTS, "/boot"), Some("ext4"));
        assert_eq!(mount_fstype(MOUNTS, "/home"), None);
    }

    #[test]
    fn last_mount_wins() {
        let mounts = format!("{MOUNTS}overlay / overlay rw 0 0\n");
        assert_eq!(mount_fstype(&mounts, "/"), Some("overlay"));
    }

    fn pci_device(root: &Path, name: &str, vendor: &str, class: &str) {
        let dev = root.join(name);
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("vendor"), format!("{vendor}\n")).unwrap();
        std::fs::write(dev.join("class"), format!("{class}\n")).unwrap();
    }

    #[test]
    fn detects_nvidia_display_controller() {
        let root = tempfile::tempdir().unwrap();
        pci_device(root.path(), "0000:00:02.0", "0x8086", "0x030000");
        assert!(!has_nvidia_display_in(root.path()));
        // NVIDIA HDMI audio function is not a display controller
        pci_device(root.path(), "0000:01:00.1", "0x10de", "0x040300");
        assert!(!has_nvidia_display_in(root.path()));
        pci_device(root.path(), "0000:01:00.0", "0x10de", "0x030000");
        assert!(has_nvidia_display_in(root.path()));
    }

    #[test]
    fn missing_pci_bus_is_not_nvidia() {
        assert!(!has_nvidia_display_in(Path::new("/nonexistent")));
    }

    #[test]
    fn reads_secure_boot_efivar() {
        let dir = tempfile::tempdir().unwrap();
        let var = dir.path().join("SecureBoot");
        std::fs::write(&var, [6, 0, 0, 0, 1]).unwrap();
        assert!(secure_boot_enabled_in(&var));
        std::fs::write(&var, [6, 0, 0, 0, 0]).unwrap();
        assert!(!secure_boot_enabled_in(&var));
        assert!(!secure_boot_enabled_in(&dir.path().join("missing")));
    }
}
