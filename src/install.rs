//! Albius Recipe generation code for Readymade
//! This module contains the code to generate a `albius::Recipe` object that can be fed into the `albius` binary.
//! So we can actually install something with Readymade.

use crate::albius::PostInstallationOperation;
use crate::disks::partition;
use crate::util::array_str_to_values;
use crate::InstallationState;
use crate::{
    albius::{Installation, Method, Mountpoint, PostInstallation, Recipe},
    disks::init::{chromebook_clean_install, clean_install, dual_boot},
    util,
};
use color_eyre::Result;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

fn determine_mountpoints(inst_type: InstallationType, disk: &Path) -> Result<Vec<Mountpoint>> {
    Ok(match inst_type {
        InstallationType::WholeDisk => vec![
            (partition(disk, 1), "/boot/efi").into(),
            (partition(disk, 2), "/boot/").into(),
            (partition(disk, 3), "/").into(),
        ],
        InstallationType::DualBoot(_) => {
            // let sdisk = format!("{}", disk.display());
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
        InstallationType::Custom => todo!(),
    })
}

fn grub_recipe(post_installation: &mut Vec<PostInstallation>, disk_str: &str) {
    let uefi = util::check_uefi();
    let params = array_str_to_values(["/boot/efi", disk_str, if uefi { "efi" } else { "bios" }]);
    if uefi {
        // assume we're on chromebooks for now
        // todo: impl this for PC UEFI installs!!
        todo!(); // TODO: figure out the boot disk
                 // append as str to params
    }

    post_installation.push(PostInstallation {
        chroot: true,
        operation: PostInstallationOperation::GrubInstall,
        params,
    })
}

fn submarine_recipe(
    post_installation: &mut Vec<PostInstallation>,
    disk: &Path,
    disk_str: String,
) -> Result<()> {
    let submarine_img = glob::glob("/usr/share/submarine/submarine-*.kpart")?
        .next()
        .expect("glob returns no results for submarine kparts")?;

    post_installation.push(PostInstallation {
        chroot: true, // for the submarine image
        operation: PostInstallationOperation::Shell,
        params: array_str_to_values([
            "dd",
            &format!("if={}", submarine_img.display()),
            &format!("of={}", partition(disk, 1).display()),
        ]),
    });
    post_installation.push(PostInstallation {
        chroot: false,
        operation: PostInstallationOperation::Shell,
        params: array_str_to_values([
            "cgpt", "add", "-i", "1", "-t", "kernel", "-P", "15", "-T", "1", "-S", "1", &disk_str,
        ]),
    });

    Ok(())
}

pub fn generate_recipe(state: &InstallationState) -> Result<Recipe> {
    let inst_type = state.installation_type.as_ref().unwrap();
    let disk = state.destination_disk.as_ref().unwrap().devpath.as_path();
    let disk_str = disk.display().to_string();

    let layout = match inst_type {
        InstallationType::WholeDisk => clean_install(disk)?,
        InstallationType::DualBoot(resize) => dual_boot(disk, *resize)?,
        InstallationType::ChromebookInstall => chromebook_clean_install(disk)?,
        InstallationType::Custom => todo!(),
    };

    let installation = Installation {
        method: Method::Unsquashfs,
        source: util::DEFAULT_SQUASH_LOCATION.to_string().into(),
        initramfs_post: vec![],
        initramfs_pre: vec![],
    };

    let mut post_installation = vec![
        PostInstallation {
            chroot: true,
            operation: PostInstallationOperation::Timezone,
            params: array_str_to_values([state.timezone.as_ref().unwrap()]),
        },
        PostInstallation {
            chroot: true,
            operation: PostInstallationOperation::Locale,
            params: array_str_to_values([state.langlocale.as_ref().unwrap()]),
        },
        PostInstallation {
            chroot: true,
            operation: PostInstallationOperation::Hostname,
            params: array_str_to_values(["ultramarine"]), // FIXME
        },
    ];
    grub_recipe(&mut post_installation, &disk_str);
    // submarine
    if let InstallationType::ChromebookInstall = inst_type {
        submarine_recipe(&mut post_installation, disk, disk_str)?;
    }

    Ok(Recipe {
        setup: layout,
        mountpoints: determine_mountpoints(inst_type.clone(), disk)?,
        installation,
        post_installation,
    })
}

#[cfg(not(debug_assertions))]
pub fn run_albius(recipe: &Recipe) -> Result<()> {
    use std::io::Write;
    let recipe_file = tempfile::Builder::new().suffix(".albius.json").tempfile()?;
    (recipe_file.as_file()).write(serde_json::to_string(recipe)?.as_bytes())?;

    let cmd = std::process::Command::new("albius")
        .arg(recipe_file.path())
        .status()?;

    let rc = cmd
        .code()
        .ok_or_else(|| color_eyre::eyre::eyre!("Failed to run albius"))?;
    if rc == 0 {
        Ok(())
    } else {
        Err(color_eyre::eyre::eyre!("Albius failed: exit code {rc}"))
    }
}

#[cfg(debug_assertions)]
pub fn run_albius(recipe: &Recipe) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(recipe)?);
    Ok(())
}
