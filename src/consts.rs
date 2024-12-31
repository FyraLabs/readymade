use std::path::PathBuf;
const EFI_SHIM_X86_64: &str = "\\EFI\\fedora\\shimx64.efi";
const EFI_SHIM_AA64: &str = "\\EFI\\fedora\\shimaa64.efi";
pub const OS_NAME: &str = "Ultramarine Linux";
pub const LIVE_BASE: &str = "/dev/mapper/live-base";
pub const ROOTFS_BASE: &str = "/run/rootfsbase";
const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";
const GRUB_BOOTLOADER_TARGET_X86: &str = "i386-pc";


pub fn repart_dir() -> PathBuf {
    PathBuf::from(std::env::var("READYMADE_REPART_DIR").unwrap_or_else(|_| REPART_DIR.into()))
}

pub const fn get_shim_path() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        EFI_SHIM_X86_64
    } else {
        EFI_SHIM_AA64
    }
}


pub const fn get_grub_bootloader_target() -> &'static str {
    if cfg!(target_arch = "x86") || cfg!(target_arch = "x86_64") {
        GRUB_BOOTLOADER_TARGET_X86
    } else {
        // XXX: This is a placeholder 
        "arm-efi"
    }
}