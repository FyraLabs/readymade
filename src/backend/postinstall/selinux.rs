use color_eyre::{eyre::bail, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::stage;

use super::{Context, PostInstallModule};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SELinux;

impl PostInstallModule for SELinux {
    fn run(&self, _context: &Context) -> Result<()> {
        stage!(selinux {
            let setfiles_cmd_status = Command::new("setfiles")
                .args(["-e", "/proc", "-e", "/sys"])
                .arg("/etc/selinux/targeted/contexts/files/file_contexts")
                .arg("/")
                .status()?;

            if !setfiles_cmd_status.success() {
                bail!(
                    "dracut failed with exit code {:?}",
                    setfiles_cmd_status.code()
                );
            }
        });

        Ok(())
    }
}
