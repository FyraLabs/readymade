use color_eyre::eyre::bail;
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};

use crate::stage;

use super::{Context, PostInstallModule};

#[derive(Clone, Debug)]
struct Grub2Defaults {
    timeout: u32,
    distributor: String,
    default: String,
    disable_submenu: bool,
    terminal_output: String,
    cmdline_linux: String,
    disable_recovery: bool,
    enable_blsconfig: bool,
}

impl Default for Grub2Defaults {
    fn default() -> Self {
        Self {
            timeout: 5,
            distributor: "$(sed 's, release .*$,,g' /etc/system-release)".to_owned(),
            default: "saved".to_owned(),
            disable_submenu: true,
            terminal_output: "console".to_owned(),
            disable_recovery: true,
            enable_blsconfig: true,
            cmdline_linux: "rhgb quiet".to_owned(),
        }
    }
}

impl Grub2Defaults {
    fn generate(&self) -> String {
        let Self {
            timeout,
            distributor,
            default,
            terminal_output,
            disable_submenu,
            cmdline_linux,
            disable_recovery,
            enable_blsconfig,
        } = self;
        format!(
            r#"GRUB_TIMEOUT={timeout}
GRUB_DISTRIBUTOR="{distributor}"
GRUB_DEFAULT={default}
GRUB_TERMINAL_OUTPUT="{terminal_output}"
GRUB_DISABLE_SUBMENU={disable_submenu}
GRUB_CMDLINE_LINUX="{cmdline_linux}"
GRUB_DISABLE_RECOVERY="{disable_recovery}"
GRUB_ENABLE_BLSCFG={enable_blsconfig}
"#
        )
    }
}

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
fn grub2_install_bios<P: AsRef<Path>>(disk: P) -> Result<()> {
    info!("Generating GRUB2 configuration...");
    let _disk = disk.as_ref().to_str();
    debug!(?_disk);
    // this should probably be run inside a chroot... but we'll see
    if let Err(e) = Command::new("grub2-mkconfig")
        .arg("-o")
        .arg("/boot/grub2/grub.cfg")
        .status()
    {
        warn!("Failed to generate GRUB2 configuration: {e}");

        // Check if the file still exists
        if !Path::new("/boot/grub/grub.cfg").exists() {
            return Err(e).map_err(Into::into);
        }
    }
    info!("Blessing the disk with GRUB2...");
    let status = Command::new("grub2-install")
        .arg("--target=i386-pc")
        .arg("--recheck")
        .arg("--boot-directory=/boot")
        // We are going to force the installation, because for some reason
        // grub-install just couldn't find our xbootldr partition
        // even though it exists.
        // 
        // --force is a last resort, but in our layout it's kind of necessary :P
        .arg("--force")
        .arg(disk.as_ref())
        .status()?;

    if !status.success() {
        bail!("Failed to install GRUB2 on disk {_disk:?}")
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GRUB2;

impl PostInstallModule for GRUB2 {
    fn run(&self, context: &Context) -> Result<()> {
        stage!("Generating system grub defaults" {
            let defaults = Grub2Defaults::default();
            let defaults_str = defaults.generate();
            std::fs::write("/etc/default/grub", defaults_str)?;
        });

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
