//! Albius Recipe generation code for Readymade
//! This module contains the code to generate a `albius::Recipe` object that can be fed into the `albius` binary.
//! So we can actually install something with Readymade.

use crate::albius::PostInstallationOperation;
use crate::disks::partition;
use crate::{
    albius::{
        DiskOperation, DiskOperationType, Installation, Method, Mountpoint, PostInstallation,
        Recipe,
    },
    disks::init::{chromebook_clean_install, clean_install, dual_boot},
    util,
};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
}

fn determine_mountpoints(inst_type: InstallationType, disk: &Path) -> Result<Vec<Mountpoint>> {
    Ok(match inst_type {
        InstallationType::WholeDisk => vec![
            (partition(disk, 1), "/boot/efi").into(),
            (partition(disk, 2), "/boot/").into(),
            (partition(disk, 3), "/").into(),
        ],
        InstallationType::DualBoot(_) => {
            let sdisk = format!("{}", disk.display());
            let last_partition = crate::disks::last_part(disk)?;
            let partn: u8 = last_partition
                .strip_suffix(|c: char| c.is_numeric())
                .expect("can't parse part number of partition")
                .parse()?;
            vec![
                (partition(disk, 1), "/boot/efi").into(), // FIXME: assumed first part is efi
                (partition(disk, partn + 1), "/boot").into(),
                (partition(disk, partn + 2), "/").into(),
            ]
        }
        InstallationType::ChromebookInstall => vec![
            // first disk is submarine
            (partition(disk, 2), "/boot/").into(),
            (partition(disk, 3), "/").into(),
        ],
    })
}

pub fn generate_recipe(inst_type: InstallationType, disk: &Path) -> Result<Recipe> {
    let layout = match inst_type {
        InstallationType::WholeDisk => clean_install(disk)?,
        InstallationType::DualBoot(resize) => dual_boot(disk, resize)?,
        InstallationType::ChromebookInstall => chromebook_clean_install(disk)?,
    };

    let installation = Installation {
        method: Method::Unsquashfs,
        source: util::DEFAULT_SQUASH_LOCATION.to_string().into(),
        initramfs_post: vec![],
        initramfs_pre: vec![],
    };

    let grub_install = {
        if util::check_uefi() {
            PostInstallation {
                chroot: true,
                operation: PostInstallationOperation::GrubInstall,
                params: vec![
                    Value::String("/boot/efi".into()),
                    Value::String(disk.to_string_lossy().into_owned()),
                    Value::String("efi".into()),
                    // Value::String(), // TODO: figure out the boot disk
                ],
            }
        } else {
            PostInstallation {
                chroot: true,
                operation: PostInstallationOperation::GrubInstall,
                params: vec![
                    Value::String("/boot/efi".into()),
                    Value::String(disk.to_string_lossy().into_owned()),
                    Value::String("bios".into()),
                ],
            }
        }
    };

    // TODO: post_installation: hdl submarine?

    Ok(Recipe {
        setup: layout,
        mountpoints: determine_mountpoints(inst_type, disk)?, //TODO
        installation,
        post_installation: vec![grub_install],
    })
}
