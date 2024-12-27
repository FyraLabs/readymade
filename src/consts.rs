use std::path::PathBuf;

pub const LIVE_BASE: &str = "/dev/mapper/live-base";
pub const ROOTFS_BASE: &str = "/run/rootfsbase";
const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";



pub fn repart_dir() -> PathBuf {
    PathBuf::from(std::env::var("READYMADE_REPART_DIR").unwrap_or(REPART_DIR.into()))
}
