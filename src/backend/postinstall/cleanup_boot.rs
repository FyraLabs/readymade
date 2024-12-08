use super::{Context, PostInstallModule};
use color_eyre::{eyre::bail, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug)]
pub struct CleanupBoot;

impl PostInstallModule for CleanupBoot {
    fn run(&self, _context: &Context) -> Result<()> {
        for file in std::fs::read_dir("/boot")?
            .flatten()
            .map(|entry| entry.path())
        {
            let file_name = file.file_name().unwrap().to_str().unwrap();
            if file_name.starts_with("initramfs") || file_name.starts_with("vmlinuz") {
                tracing::debug!(?file, "Removing kernel file");
                std::fs::remove_file(file)?;
            }
        }

        for file in std::fs::read_dir("/boot/loader/entries")?
            .flatten()
            .map(|entry| entry.path())
        {
            tracing::debug!(?file, "Removing BLS entry");
            std::fs::remove_file(file)?;
        }

        Ok(())
    }
}
