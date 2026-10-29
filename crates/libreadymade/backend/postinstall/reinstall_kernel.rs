use super::{Context, PostInstallModule};
use crate::stage;
use color_eyre::{Result, eyre::bail};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReinstallKernel;

impl PostInstallModule for ReinstallKernel {
    fn name(&self) -> &'static str {
        "ReinstallKernel"
    }

    fn run(&self, _context: &Context) -> Result<()> {
        // Only directories with a kernel image are installed kernels; others may be left over
        // from e.g. out-of-tree modules
        let mut kernel_vers = vec![];
        for entry in std::fs::read_dir("/lib/modules")? {
            let entry = entry?;
            if entry.path().join("vmlinuz").is_file() {
                kernel_vers.push(entry.file_name());
            }
        }
        kernel_vers.sort();

        tracing::info!(?kernel_vers, "Kernel versions found");

        if kernel_vers.is_empty() {
            bail!("No kernels found in /lib/modules");
        }

        // CleanupBoot removes every kernel from /boot, so reinstall all of them
        stage!(kernel "Reinstalling kernels" {
            for kver in kernel_vers {
                let kernel_install_cmd_status = Command::new("kernel-install")
                    .arg("add")
                    .arg(&kver)
                    .arg(std::path::Path::new("/lib/modules").join(&kver).join("vmlinuz"))
                    .arg("--verbose")
                    .status()?;

                if !kernel_install_cmd_status.success() {
                    bail!(
                        "kernel-install for {kver:?} failed with exit code {:?}",
                        kernel_install_cmd_status.code()
                    );
                }
            }
        });

        Ok(())
    }
}
