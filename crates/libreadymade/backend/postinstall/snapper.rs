//! Turns on snapper snapshots of the root filesystem for mutable installs.
//!
//! The image ships snapper and a libdnf5 actions hook that takes a pre/post snapshot pair around
//! every dnf transaction, but only once `/etc/snapper/configs/root` exists. This module creates
//! that config when `/` is btrfs, and does nothing otherwise.
//!
//! `/home` and `/var` live on their own subvolumes (see the `mutablewholedisk` repart template),
//! so root snapshots don't include them and rolling back never rewinds app data. Roll back to
//! before a transaction with `snapper undochange <pre>..0`, then run
//! `ultramarine-prune-boot-entries` to drop boot entries for kernels whose modules the rollback
//! removed. Don't use `<pre>..<post>`: the post snapshot is taken while dnf still has the RPM
//! database open, so the change is only in its SQLite WAL and `rpmdb.sqlite` doesn't get restored.
//!
//! Run this before [`super::selinux::SELinux`] so `/.snapshots` gets labeled.

use crate::{backend::util::sys::mount_fstype, prelude::*, stage};

use super::{Context, PostInstallModule};

const SNAPPER: &str = "/usr/bin/snapper";
const ROOT_CONFIG: &str = "/etc/snapper/configs/root";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Snapper;

impl PostInstallModule for Snapper {
    fn name(&self) -> &'static str {
        "Snapper"
    }

    fn run(&self, _context: &Context) -> Result<()> {
        if !Path::new(SNAPPER).exists() {
            tracing::info!("snapper is not installed, skipping root snapshots");
            return Ok(());
        }
        let mounts = std::fs::read_to_string("/proc/self/mounts")
            .wrap_err("cannot read /proc/self/mounts")?;
        let fstype = mount_fstype(&mounts, "/");
        if fstype != Some("btrfs") {
            tracing::info!(?fstype, "/ is not btrfs, skipping root snapshots");
            return Ok(());
        }
        if Path::new(ROOT_CONFIG).exists() {
            tracing::info!("snapper root config already exists");
            return Ok(());
        }

        stage!(snapper "Setting up root filesystem snapshots" {
            // There's no snapperd to talk to inside the install chroot
            crate::cmd!("snapper" [["--no-dbus", "create-config", "/"]]
                => |r| bail!("snapper create-config failed with exit code {:?}", r.code()));
        });
        Ok(())
    }
}
