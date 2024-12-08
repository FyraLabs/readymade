use color_eyre::eyre::bail;
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tracing::{info, warn};

use crate::{
    stage,
    util::{self, run_as_root},
};

use super::{Context, PostInstallModule};

/// Helper function to install GRUB2 on a legacy BIOS system.
///
/// You should run this inside a [`tiffin::Container`].
///
/// This function runs `grub2-mkconfig` and `grub2-install` to install GRUB2 on a legacy BIOS system.
///
/// NOTE: To successfully install GRUB on a legacy BIOS system, you need to be running on
/// an IBM PC-compatible system with an older BIOS firmware. If you are running on a UEFI system,
/// please refer to the standard UEFI installation method.
///
/// You will also require a small, blank GPT partition for the BIOS boot partition so the MBR headers
/// have a place to live. This partition should be at least 1MB in size.
///
/// This function will attempt to generate a GRUB configuration and then write the bootloader directly to the header
/// of the disk, which should be allocated to that small BIOS boot partition.
///
/// # Arguments
///
/// * `disk` - The path to the disk to install GRUB2 on.
fn grub2_install_bios<P: AsRef<Path>>(disk: P) -> std::io::Result<()> {
    info!("Generating GRUB2 configuration...");
    // this should probably be run inside a chroot... but we'll see
    if let Err(e) = run_as_root("grub2-mkconfig -o /boot/grub/grub.cfg") {
        warn!("Failed to generate GRUB2 configuration: {e}");

        // Check if the file still exists
        if !Path::new("/boot/grub/grub.cfg").exists() {
            return Err(e);
        }
    }
    info!("Blessing the disk with GRUB2...");
    run_as_root(&format!(
        "grub2-install --target=i386-pc --recheck --boot-directory=/boot {}",
        disk.as_ref().display()
    ))?;

    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GRUB2;

impl PostInstallModule for GRUB2 {
    fn run(&self, context: &Context) -> Result<()> {
        if context.uefi {
            // The reason why we don't do grub2-install here is because for
            // Fedora specifically, the install script simply plops in
            // a pre-built GRUB binary in the ESP that looks for the stage 1
            // config in /boot/efi/EFI/fedora/grub.cfg
            // The following config then redirects to the actual stage 2 config located
            // in /boot/grub2/grub.cfg
            // This is actually done to support BLS entries properly on their end

            // todo: Add support for systemd-boot
            std::fs::create_dir_all("/boot/efi/EFI/fedora")?;

            stage!("Generating stage 1 grub.cfg in ESP..." {
                let mut grub_cfg = std::fs::File::create("/boot/efi/EFI/fedora/grub.cfg")?;
                // let's search for an xbootldr label
                // because we never know what the device will be
                // see the compile time included config file
                grub_cfg.write_all(include_bytes!("../../../templates/fedora-grub.cfg"))?;
            });

            stage!("Generating stage 2 grub.cfg in /boot/grub2/grub.cfg..." {
                let grub_cmd_status = Command::new("grub2-mkconfig")
                    .arg("-o")
                    .arg("/boot/grub2/grub.cfg")
                    .status()?;

                if !grub_cmd_status.success() {
                    bail!("grub2-mkconfig failed with exit code {:?}", grub_cmd_status.code());
                }
            });
        } else {
            stage!("Installing BIOS Grub2" {
                grub2_install_bios(&context.destination_disk)?;
            });
        }

        Ok(())
    }
}
