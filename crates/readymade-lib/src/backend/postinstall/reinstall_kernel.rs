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
            .map(|entry| entry.unwrap().file_name())
            .collect_vec();

        tracing::info!(?kernel_vers, "Kernel versions found");

        // We're gonna just install the first kernel we find, so let's do that
        let kver = kernel_vers.first().unwrap().to_str().unwrap();

        // install kernel

        stage!(kernel {
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
        });

        Ok(())
    }
}
