use super::{Context, PostInstallModule};
use crate::prelude::*;
use color_eyre::Result;
use color_eyre::eyre::OptionExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Fstab;

impl PostInstallModule for Fstab {
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
/// This function may be deprecated when systemd 256 hits f40, or when
/// we rebase to f41
///
/// # XXX: This will read the config files in the ***CURRENT*** root context
/// So you must have the same config files in the chroot, or exit the chroot,
/// run this, then re-enter the chroot.
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

    let bufreader = std::fs::read_to_string("/proc/mounts").wrap_err("cannot open /proc/mounts")?;
    // BufReader::from(std::fs::File::open("/proc/mounts").wrap_err("cannot open /proc/mounts")?);
    let mut fstypes = HashMap::new();
    for line in bufreader.lines() {
        let [_, mount, parttype, ..] = line.split(" ").collect::<Vec<_>>()[..] else {
            panic!("I'm not reading /proc/mounts?");
        };
        fstypes.insert(mount, parttype);
    }

    for ((mnt, part), mountobj) in partitions {
        println!("Processing partition: {}", part.display());
        // if let Some(_mntpnt) = part.ddi_mountpoint() {
        tracing::trace!(?part, "Processing partition");

        let entry = fstab_entry(
            mountobj.clone(),
            fstypes[&format!("{}", mnt.display()).as_str()],
        )?;
        writeln!(&mut fstab, "{entry}").unwrap();
    }

    Ok(fstab)
}

/// Generate an FS Table entry for the partition,
/// Returns a line for /etc/fstab
///
/// This will refer to the config file systemd-repart refers to.
///
#[tracing::instrument]
pub fn fstab_entry(mount: Mount, fstype: &str) -> color_eyre::Result<String> {
    const FALLBACK_FS: &str = "auto";
    const FALLBACK_OPTS: &str = "defaults";
    const FALLBACK_DUMP: i32 = 0;
    const FALLBACK_PASS: i32 = 2;

    tracing::trace!("Generating fstab entry");
    // get fs type
    let fs_fmt = fstype;

    // serialize fs into string, if it's not there, use the fallback
    // use serde::Serialize;
    let fs_fmt_str = serde_json::to_string(fs_fmt)
        .unwrap_or_else(|_| FALLBACK_FS.to_owned())
        .replace('"', "");

    let mut mount_opts = String::new();

    if let opts = &mount.options {
        write!(&mut mount_opts, "{opts}")?;
    }

    // if we still have no mount options, use the fallback

    if mount_opts.is_empty() {
        FALLBACK_OPTS.clone_into(&mut mount_opts);
    }

    // let's get the UUID

    // let uuid = self.uuid.to_string();
    // Check if the disk is encrypted
    let is_encrypted = crate::backend::mounts::is_luks(&mount.partition);
    let uuid_string = if is_encrypted {
        tracing::trace!("Partition is encrypted");
        // We're gonna do what's called a pro gamer move.
        // HACK: We will guess the UUID of the decrypted LUKS partition by:
        // - Guessing where the mapper device will be
        // - Finding the UUID of the mapper device by doing some symlink magic (thanks udev!)
        // - Using that UUID for the fstab entry

        // We're gonna be abusing the mapper cache, which should be populated by the time we get here

        let mapper_cache = crate::backend::mounts::MAPPER_CACHE.read();
        let mapper_path = mapper_cache
            .get(&format!("{}", mount.partition.display()))
            .unwrap();

        tracing::trace!(?mapper_path, "Guessed mapper path as this");

        // Thankfully, since we made lsblk-rs we can do this easily.
        let device = lsblk::BlockDevice::from_path(mapper_path)?;
        drop(mapper_cache);
        tracing::trace!(?device, "Found device from mapper path");
        let uuid = device
            .uuid
            .ok_or_eyre("Could not find UUID of decrypted device")?;

        // The mapper path should be a symlink to the /dev/dm-XX device

        // let dm = std::fs::read_link(&mapper_path)?;

        // tracing::trace!(?dm, "Found decrypted device");

        // let uuid = std::fs::read_dir("/dev/disk/by-uuid")?
        //     .find_map(|entry| {
        //         let entry = entry.ok()?;
        //         let path = entry.path();
        //         let link = std::fs::read_link(&path).ok()?;
        //         if link == dm {
        //             Some(path.file_name()?.to_string_lossy().to_string())
        //         } else {
        //             None
        //         }
        //     })
        //     .ok_or_eyre("Could not find UUID for decrypted device")?;

        // tracing::trace!(?uuid, "Found UUID for decrypted device!");

        format!("UUID={uuid}")
    } else {
        tracing::trace!("Partition is not encrypted, using repart's");
        format!(
            "UUID={}",
            lsblk::BlockDevice::from_path(&mount.partition)?
                .uuid
                .ok_or_eyre("can't find uuid of device")?
        )
    };

    // let's get the dump and pass values

    let dump = FALLBACK_DUMP; //todo: is there a config option for this?

    // We will be checking from filesystem type
    // or the root device it should be 1. For other partitions it should be 2, or 0 to disable checking.
    // If the root file system is btrfs or XFS, the fsck order should be set to 0 instead of 1.
    let pass = match fs_fmt {
        "btrfs" | "xfs" => 0,
        _ if mount.mountpoint.to_str().unwrap() == "/" => 1,
        _ => FALLBACK_PASS,
    };

    Ok(format!(
        "{uuid_string}\t{}\t{fs_fmt_str}\t{mount_opts}\t{dump}\t{pass}",
        mount.mountpoint.display()
    ))
}
