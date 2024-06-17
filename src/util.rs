//! QoL Utilities for Readymade
use bytesize::ByteSize;
use color_eyre::Section as _;

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

pub fn chain_err<E: std::error::Error + Send + Sync + 'static>(
    msg: &'static str,
) -> impl FnOnce(E) -> color_eyre::Report {
    move |e| color_eyre::Report::msg(msg).error(e)
}

pub fn make_push<T>(mut vector: Vec<T>, elm: T) -> Vec<T> {
    vector.push(elm);
    vector
}

#[inline]
pub(crate) fn array_str_to_values<const N: usize>(arr: [&str; N]) -> Vec<serde_json::Value> {
    arr
    .into_iter()
    .map(ToString::to_string)
    .map(serde_json::Value::String)
    .collect()
}
