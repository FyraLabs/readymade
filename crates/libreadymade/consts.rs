use std::path::PathBuf;
const EFI_SHIM_X86_64: &str = "\\EFI\\fedora\\shimx64.efi";
const EFI_SHIM_AA64: &str = "\\EFI\\fedora\\shimaa64.efi";
pub const LIVE_BASE: &str = "/dev/mapper/live-base";
pub const ROOTFS_BASE: &str = "/run/rootfsbase";
pub const LUKS_KEYFILE_PATH: &str = "/run/readymade-luks.key";
const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";
pub const READYMADE_STATE_PATH: &str = "/var/lib/readymade/state.json";

pub fn repart_dir() -> PathBuf {
    PathBuf::from(std::env::var("READYMADE_REPART_DIR").unwrap_or_else(|_| REPART_DIR.into()))
}

pub const fn shim_path() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        EFI_SHIM_X86_64
    } else {
        EFI_SHIM_AA64
    }
}
