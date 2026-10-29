use super::{Context, PostInstallModule};
use crate::prelude::*;
use std::collections::HashMap;
use std::fmt::Write;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Fstab;

impl PostInstallModule for Fstab {
    fn name(&self) -> &'static str {
        "Fstab"
    }

    fn run(&self, context: &Context) -> Result<()> {
        tracing::info!("Writing /etc/fstab...");
        let fstab = generate_fstab(&context.mounts).wrap_err("cannot generate fstab")?;
        std::fs::create_dir_all("/etc/").wrap_err("cannot create /etc/")?;
        std::fs::write("/etc/fstab", fstab).wrap_err("cannot write to /etc/fstab")?;
        Ok(())
    }
}

/// Generate a /etc/fstab file from the DDI partition types
///
/// The filesystem types are read from `/proc/mounts`, so `mounts` must be mounted relative to the
/// current root (i.e. this should run inside the installed system's chroot).
///
/// # Errors
///
/// Returns an error if `/proc/mounts` cannot be read or parsed, a partition is not mounted, or
/// an entry cannot be generated.
pub fn generate_fstab(mounts: &Mounts) -> color_eyre::Result<String> {
    let mut fstab = String::new();

    let mut partitions = mounts
        .0
        .iter()
        .map(|m| ((m.mountpoint.clone(), m.partition.clone()), m))
        .collect_vec();
    // sort by mountpoint,
    // root goes first, then each subdirectory counting the slashes
    partitions.sort_by(|((a_mnt, _), _), ((b_mnt, _), _)| {
        let a_mnt = format!("{}", a_mnt.display());
        let b_mnt = format!("{}", b_mnt.display());
        // If either path is root (/), it should go first
        if a_mnt == "/" {
            std::cmp::Ordering::Less
        } else if b_mnt == "/" {
            std::cmp::Ordering::Greater
        } else {
            // Otherwise sort by number of slashes then alphabetically
            let a_slashes = a_mnt.chars().filter(|&c| c == '/').count();
            let b_slashes = b_mnt.chars().filter(|&c| c == '/').count();
            a_slashes.cmp(&b_slashes).then(a_mnt.cmp(&b_mnt))
        }
    });

    tracing::trace!(?partitions, "Sorted partitions");

    let proc_mounts =
        std::fs::read_to_string("/proc/mounts").wrap_err("cannot open /proc/mounts")?;
    let mut fstypes = HashMap::new();
    for line in proc_mounts.lines() {
        let [_, mount, parttype, ..] = line.split(' ').collect::<Vec<_>>()[..] else {
            bail!("Unexpected line in /proc/mounts: {line:?}");
        };
        // mount points with whitespace in them are octal-escaped, like in fstab
        fstypes.insert(filesystem_table::unescape_field(mount), parttype);
    }

    for ((mnt, part), mountobj) in partitions {
        tracing::trace!(?part, "Processing partition");

        let mnt = format!("{}", mnt.display());
        let fstype = fstypes
            .get(mnt.as_str())
            .ok_or_else(|| eyre!("{mnt} is not mounted, cannot determine its filesystem type"))?;
        let entry = fstab_entry(mountobj, fstype)?;
        writeln!(&mut fstab, "{entry}")?;
    }

    Ok(fstab)
}

/// Generate an FS Table entry for the partition,
/// Returns a line for /etc/fstab
///
/// # Errors
///
/// Returns an error if the UUID of the partition (or, for LUKS partitions, its decrypted mapper
/// device) cannot be found.
#[tracing::instrument]
pub fn fstab_entry(mount: &Mount, fstype: &str) -> color_eyre::Result<String> {
    const FALLBACK_OPTS: &str = "defaults";
    const FALLBACK_DUMP: i32 = 0;
    const FALLBACK_PASS: i32 = 2;

    tracing::trace!("Generating fstab entry");

    // if we have no mount options, use the fallback
    let mount_opts = if mount.options.is_empty() {
        FALLBACK_OPTS
    } else {
        mount.options.as_str()
    };

    // fstab refers to the filesystem, which for LUKS partitions is on the decrypted mapper device
    let device = if crate::backend::mounts::is_luks(&mount.partition) {
        tracing::trace!("Partition is encrypted");
        // The partition has been decrypted to mount it, so it's in the mapper cache
        crate::backend::mounts::MAPPER_CACHE
            .read()
            .get(&*mount.partition.to_string_lossy())
            .cloned()
            .ok_or_else(|| eyre!("{} has not been decrypted", mount.partition.display()))?
    } else {
        mount.partition.clone()
    };
    tracing::trace!(?device, "Getting UUID of filesystem");
    let uuid = lsblk::BlockDevice::from_path(&device)?
        .uuid
        .ok_or_else(|| eyre!("can't find UUID of {}", device.display()))?;

    let dump = FALLBACK_DUMP; //todo: is there a config option for this?

    // We will be checking from filesystem type
    // or the root device it should be 1. For other partitions it should be 2, or 0 to disable checking.
    // If the root file system is btrfs or XFS, the fsck order should be set to 0 instead of 1.
    let pass = match fstype {
        "btrfs" | "xfs" => 0,
        _ if mount.mountpoint == std::path::Path::new("/") => 1,
        _ => FALLBACK_PASS,
    };

    Ok(format!(
        "UUID={uuid}\t{}\t{fstype}\t{mount_opts}\t{dump}\t{pass}",
        filesystem_table::escape_field(&mount.mountpoint.to_string_lossy())
    ))
}
