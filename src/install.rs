//! Albius Recipe generation code for Readymade
//! This module contains the code to generate a `albius::Recipe` object that can be fed into the `albius` binary.
//! So we can actually install something with Readymade.

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
use crate::albius::PostInstallationOperation;
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
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

    // todo: SOMEHOW FIGURE OUT THE BOOT DISK??


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


    let recipe = Recipe {
        setup: layout,
        mountpoints: vec![], //TODO
        installation,
        post_installation: vec![],
    };

    // im so sorry
    todo!()
}
