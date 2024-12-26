use color_eyre::{eyre::bail, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::stage;

use super::{Context, PostInstallModule};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReinstallKernel;

impl PostInstallModule for ReinstallKernel {
    #[allow(clippy::unwrap_in_result)]
    fn run(&self, context: &Context) -> Result<()> {
        let kernel_vers = std::fs::read_dir("/lib/modules")?
            .map(|entry| entry.unwrap().file_name())
            .collect_vec();

        tracing::info!(?kernel_vers, "Kernel versions found");

        // We're gonna just install the first kernel we find, so let's do that
        let kver = kernel_vers.first().unwrap().to_str().unwrap();

        // install kernel

        let kernel_install_cmd_status = Command::new("kernel-install")
            .arg("add")
            .arg(kver)
            .arg(format!("/lib/modules/{kver}/vmlinuz"))
            .arg("--verbose")
            .status()?;

        if !kernel_install_cmd_status.success() {
            bail!(
                "kernel-install failed with exit code {:?}",
                kernel_install_cmd_status.code()
            );
        }

        // HACK: now let's edit the BLS boot entries
        // for some reason it points to the wrong root partition UUID

        stage!("Correcting BLS entries" {

            for file in std::fs::read_dir("/boot/loader/entries")?
            .flatten()
            .map(|entry| entry.path())
        {
            tracing::debug!(?file, "File");
            // open the file and do some regex editing
            let root_uuid = std::process::Command::new("lsblk")
                .arg("-no")
                .arg("UUID")
                .arg(context.destination_disk.as_os_str())
                .output()?;

            // HACK: We're gonna also add `rhgb quiet` to the kernel command line
            // XXX: Please remove this when we have a proper way to append to cmdline >_<
            let root_cmdline = format!("root=UUID={} rhgb quiet", String::from_utf8(root_uuid.clone().stdout).unwrap());

            tracing::debug!(?root_uuid, "Root UUID");
            let file_contents = std::fs::read_to_string(&file)?;
            tracing::trace!(?file_contents, "File contents");
            // regex replace the root=UUID=... with the correct UUID
            let regex = regex::Regex::new("root=UUID=[a-f0-9-]+")?;
            let new_contents = regex.replace(
                &file_contents,
                root_cmdline.as_str(),
            );
            tracing::trace!(?new_contents, "New contents");

            std::fs::write(&file, new_contents.as_bytes())?;
        }

        });

        Ok(())
    }
}
