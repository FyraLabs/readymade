use super::{Context, PostInstallModule};
use crate::stage;
use color_eyre::{eyre::bail, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReinstallKernel;

impl PostInstallModule for ReinstallKernel {
    fn run(&self, _context: &Context) -> Result<()> {
        let kernel_vers = std::fs::read_dir("/lib/modules")?
            .filter_map(|entry| entry.ok().map(|e| e.file_name()))
            .collect_vec();

        tracing::info!(?kernel_vers, "Kernel versions found");

        // We're gonna just install the first kernel we find, so let's do that
        let kver = kernel_vers
            .first()
            .ok_or_else(|| color_eyre::eyre::eyre!("No kernel versions found in /lib/modules"))?
            .to_str()
            .ok_or_else(|| color_eyre::eyre::eyre!("Kernel version filename is not valid UTF-8"))?;

        // install kernel
        let vmlinuz_path = format!("/lib/modules/{kver}/vmlinuz");
        if !std::path::Path::new(&vmlinuz_path).exists() {
            bail!("Kernel version {kver} does not have a vmlinuz file at {vmlinuz_path}");
        }

        stage!(kernel {
            let kernel_install_cmd_status = Command::new("kernel-install")
                .arg("add")
                .arg(kver)
                .arg(&vmlinuz_path)
                .arg("--verbose")
                .status()?;

            if !kernel_install_cmd_status.success() {
                bail!(
                    "kernel-install failed with exit code {:?}",
                    kernel_install_cmd_status.code()
                );
            }
        });

        stage!(recovery {
            // copy to /boot/vmlinuz-recovery
            let recovery_vmlinuz_path = "/boot/vmlinuz-recovery".to_owned();
            if std::path::Path::new(&recovery_vmlinuz_path).exists() {
                tracing::warn!("Recovery kernel already exists at {recovery_vmlinuz_path}, skipping copy");
            } else {
                std::fs::copy(&vmlinuz_path, &recovery_vmlinuz_path)
                    .map_err(|e| color_eyre::eyre::eyre!(e))?;
            }

            // create a recovery initramfs
            let recovery_initramfs_path = "/boot/initramfs-recovery.img".to_owned();

            let recovery_dracut = Command::new("dracut")
                .arg("--force")
                .arg("--add")
                .arg("dmsquash-live overlayfs rescue")
                .arg("--no-hostonly")
                .arg("--no-uefi")
                .arg("--kver")
                .arg(kver)
                .arg(&recovery_initramfs_path)
                .status()?;
            if !recovery_dracut.success() {
                bail!(
                    "dracut failed with exit code {:?}",
                    recovery_dracut.code()
                );
            }
            tracing::info!("Recovery initramfs created at {recovery_initramfs_path}");
        });
        // todo: grub.d template for boot entry in OS

        Ok(())
    }
}
