use super::{Context, PostInstallModule};
use crate::backend::util::fs::exist_then_read_dir;
use color_eyre::Result;
use serde::{Deserialize, Serialize};

/// Remove the kernels, initramfs images and BLS entries copied from the live system from `/boot`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CleanupBoot;

impl PostInstallModule for CleanupBoot {
    fn name(&self) -> &'static str {
        "CleanupBoot"
    }

    fn run(&self, _context: &Context) -> Result<()> {
        for entry in std::fs::read_dir("/boot")?.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if (file_name.starts_with("initramfs") || file_name.starts_with("vmlinuz"))
                && entry.file_type().is_ok_and(|t| !t.is_dir())
            {
                let file = entry.path();
                tracing::debug!(?file, "Removing kernel file");
                std::fs::remove_file(file)?;
            }
        }

        for file in exist_then_read_dir("/boot/loader/entries")?.map(|entry| entry.path()) {
            tracing::debug!(?file, "Removing BLS entry");
            std::fs::remove_file(file)?;
        }

        Ok(())
    }
}
