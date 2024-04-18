//! QoL Utilities for Readymade
use bytesize::ByteSize;

pub const MAX_EFI_SIZE: ByteSize = ByteSize::gb(1);
pub const DEFAULT_SQUASH_LOCATION: &str = "/run/initramfs/live/LiveOS/squashfs.img";

#[cfg(target_os = "linux")]
/// Check if the current running system is UEFI or not.
pub fn check_uefi() -> bool {
    std::fs::read_to_string("/sys/firmware/efi").is_ok()
}

// macro to wrap around cmd_lib::run_fun! to prepend pkexec if not root

#[cfg(target_os = "linux")]
/// Run a command with elevated privileges if not already root.
pub fn run_as_root(cmd: &str) -> Result<String, std::io::Error> {
    if !cmd_lib::run_fun!("whoami").unwrap().contains("root") {
        cmd_lib::run_fun!(pkexec $cmd)
    } else {
        cmd_lib::run_fun!($cmd)
    }
}

// Also, fail compilation on non-Linux platforms
#[cfg(not(target_os = "linux"))]
compile_error!(
    "Readymade does not support non-Linux platforms, these functions are Linux-specific."
);
