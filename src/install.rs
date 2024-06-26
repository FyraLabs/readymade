use color_eyre::Result;
use std::path::{Path, PathBuf};

const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";

#[derive(Debug, Clone)]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

impl InstallationType {
    pub fn install(&self, state: &crate::InstallationState) -> Result<()> {
        let blockdev = &state.destination_disk.as_ref().unwrap().devpath;
        let cfgdir = self.cfgdir();
        Self::systemd_repart(blockdev, &cfgdir)?;
        if let Self::ChromebookInstall = self {
            Self::set_cgpt_flags(blockdev)?;
        }
        Ok(())
    }
    fn cfgdir(&self) -> PathBuf {
        match self {
            Self::ChromebookInstall => const_format::concatcp!(REPART_DIR, "chromebookinstall"),
            _ => todo!(),
        }
        .into()
    }
    fn systemd_repart(blockdev: &Path, cfgdir: &Path) -> Result<()> {
        let dry_run = if cfg!(debug_assertions) { "yes" } else { "no" };
        cmd_lib::run_cmd!(
            systemd-repart
                --dry-run=$dry_run
                --definitions=$cfgdir
                $blockdev
        )?;
        Ok(())
    }
    fn set_cgpt_flags(blockdev: &Path) -> Result<()> {
        cmd_lib::run_cmd!(cgpt add -i 1 -t kernel -P 15 -T 1 -S 1 $blockdev)?;
        Ok(())
    }
}
