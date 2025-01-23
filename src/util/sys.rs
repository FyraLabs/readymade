/// Check if the current running system is UEFI or not.
///
/// Simply checks for the existence of the `/sys/firmware/efi` directory.
///
/// False negatives are possible if the system is booted in BIOS mode and the UEFI variables are not exposed.
pub fn check_uefi() -> bool {
    std::fs::metadata("/sys/firmware/efi").is_ok()
}

/// Check if the current running system is a Chromebook device.
///
/// This function simply checks the existence of support for the ChromeOS embedded controller.
///
/// If the current system exposes a ChromeOS EC device, it is assumed to be a Chromebook.
///
/// There should never be a false positive since the EC device is exclusively used by Chromebooks,
/// But false negatives are possible if the device is not exposed to the current system
/// (e.g running in a VM or a container, or using a really old kernel without the I2C EC driver).
#[cfg(target_os = "linux")]
pub fn is_chromebook() -> bool {
    std::fs::metadata("/dev/cros_ec").is_ok()
}
