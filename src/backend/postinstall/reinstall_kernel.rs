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

        stage!("Correcting Kernel arguments" {
            let root_uuid = std::process::Command::new("lsblk")
                .arg("-no")
                .arg("UUID")
                .arg(context.destination_disk.as_os_str())
                .output()?;

            let root_cmdline = format!("root=UUID={} rhgb quiet", String::from_utf8(root_uuid.stdout).unwrap());

            // now append/create the cmdline file in /etc/kernel/cmdline (file)

            let mut file_handle = {
                if std::fs::metadata("/etc/kernel/cmdline").is_ok() {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open("/etc/kernel/cmdline")?
                } else if std::fs::create_dir_all("/etc/kernel").is_err() {
                    bail!("Failed to create /etc/kernel directory");
                } else {
                    std::fs::File::create("/etc/kernel/cmdline")?
                }
            };

            std::io::Write::write_all(&mut file_handle, root_cmdline.as_bytes())?;
        });

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

        Ok(())
    }
}
