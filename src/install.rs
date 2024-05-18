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

fn grub_recipe(post_installation: &mut Vec<PostInstallation>, disk_str: &str) {
    let uefi = util::check_uefi();
    let mut params = vec![
        Value::String("/boot/efi".into()),
        Value::String(disk_str.to_string()),
        Value::String(if uefi { "efi" } else { "bios" }.into()),
    ];
    if uefi {
        todo!(); // TODO: figure out the boot disk
                 // append as str to params
    }

    post_installation.push(PostInstallation {
        chroot: true,
        operation: PostInstallationOperation::GrubInstall,
        params,
    })
}

fn submarine_recipe(post_installation: &mut Vec<PostInstallation>, disk: &Path, disk_str: String) {
    post_installation.push(PostInstallation {
        chroot: true, // for the submarine image
        operation: PostInstallationOperation::Shell,
        params: vec![
            Value::String("dd".into()),
            Value::String("if=/usr/share/submarine/submarine-*.kpart".into()),
            Value::String(format!("of={}", partition(disk, 1).display())),
        ],
    });
    post_installation.push(PostInstallation {
        chroot: false,
        operation: PostInstallationOperation::Shell,
        params: vec![
            Value::String("cgpt".into()),
            Value::String("add".into()),
            Value::String("-i".into()),
            Value::String("1".into()),
            Value::String("-t".into()),
            Value::String("kernel".into()),
            Value::String("-P".into()),
            Value::String("15".into()),
            Value::String("-T".into()),
            Value::String("1".into()),
            Value::String("-S".into()),
            Value::String("1".into()),
            Value::String(disk_str.to_string()),
        ],
    });
}

pub fn generate_recipe(inst_type: InstallationType, disk: &Path) -> Result<Recipe> {
    let disk_str = disk.display().to_string();

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

    let mut post_installation = vec![];
    grub_recipe(&mut post_installation, &disk_str);
    // submarine
    if let InstallationType::ChromebookInstall = inst_type {
        submarine_recipe(&mut post_installation, disk, disk_str);
    }

    Ok(Recipe {
        setup: layout,
        mountpoints: determine_mountpoints(inst_type, disk)?,
        installation,
        post_installation,
    })
}
