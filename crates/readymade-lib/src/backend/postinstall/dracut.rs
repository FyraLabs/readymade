use crate::stage;

use super::{Context, PostInstallModule};
use color_eyre::{eyre::bail, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Dracut;

impl PostInstallModule for Dracut {
    fn run(&self, _context: &Context) -> Result<()> {
        stage!(initramfs {
            // We assume the installation wouldn't be used on another system (false only if you install
            // on something like a USB stick anyway)
            // → reduce size of initramfs aggressively for faster boot times
            //
            // on my system this reduces the size from 170M down to 43M.
            // — mado
            let dracut_cmd_status = Command::new("dracut")
                .args([
                    "--force",
                    "--parallel",
                    "--regenerate-all",
                    "--hostonly",
                    "--strip",
                    "--aggressive-strip",
                ])
                .status()?;

            if !dracut_cmd_status.success() {
                bail!(
                    "dracut failed with exit code {:?}",
                    dracut_cmd_status.code()
                );
            }
        });

        Ok(())
    }
}
