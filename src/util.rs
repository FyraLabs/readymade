//! QoL Utilities for Readymade
use bytesize::ByteSize;
use color_eyre::Section as _;

pub const MAX_EFI_SIZE: ByteSize = ByteSize::gb(1);
pub const DEFAULT_SQUASH_LOCATION: &str = "/run/initramfs/live/LiveOS/squashfs.img";

#[cfg(target_os = "linux")]
/// Check if the current running system is UEFI or not.
///
/// Simply checks for the existence of the `/sys/firmware/efi` directory.
///
/// False negatives are possible if the system is booted in BIOS mode and the UEFI variables are not exposed.
pub fn check_uefi() -> bool {
    std::fs::read_to_string("/sys/firmware/efi").is_ok()
}

// macro to wrap around cmd_lib::run_fun! to prepend pkexec if not root

#[cfg(target_os = "linux")]
/// Run a command with elevated privileges if not already root.
///
/// This function relies upon the `pkexec` command to elevate privileges.
///
/// If the current user is not root, the command will be run with `pkexec`.
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
/// Chain an error with a custom message.
pub fn chain_err<E: std::error::Error + Send + Sync + 'static>(
    msg: &'static str,
) -> impl FnOnce(E) -> color_eyre::Report {
    move |e| color_eyre::Report::msg(msg).error(e)
}

/// Internal function to append an element to a vector.
pub fn make_push<T>(mut vector: Vec<T>, elem: T) -> Vec<T> {
    vector.push(elem);
    vector
}

/// Internal function to convert an array of `&str` to an array of `serde_json::Value`.
#[inline]
pub(crate) fn array_str_to_values<const N: usize>(arr: [&str; N]) -> Vec<serde_json::Value> {
    arr.into_iter()
        .map(ToString::to_string)
        .map(serde_json::Value::String)
        .collect()
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
