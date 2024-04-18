//! QoL Utilities for Readymade
use bytesize::ByteSize;

pub const MAX_EFI_SIZE: ByteSize = ByteSize::gb(1);
pub const DEFAULT_SQUASH_LOCATION: &str = "/run/initramfs/live/LiveOS/squashfs.img";

#[cfg(target_os = "linux")]
/// Check if the current running system is UEFI or not.
pub fn check_uefi() -> bool {
    std::fs::read_to_string("/sys/firmware/efi").is_ok()
}

// Also, fail compilation on non-Linux platforms
#[cfg(not(target_os = "linux"))]
compile_error!(
    "Readymade does not support non-Linux platforms, these functions are Linux-specific."
);
