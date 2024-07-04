use color_eyre::Result;
use std::path::{Path, PathBuf};
use sys_mount::Unmount;

const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";

#[derive(Debug, Clone)]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

impl InstallationType {
    #[tracing::instrument]
    pub fn install(&self, state: &crate::InstallationState) -> Result<()> {
        let blockdev = &state.destination_disk.as_ref().unwrap().devpath;
        let cfgdir = self.cfgdir();
        // todo: revert the backhand removal, use it to unsquash

        Self::systemd_repart(blockdev, &cfgdir)?;
        if let Self::ChromebookInstall = self {
            // todo: not freeze on error, show error message as err handler?
            Self::set_cgpt_flags(blockdev)?;
        }
        tracing::info!("install() finished");
        Ok(())
    }
    fn mount_squashimg() -> std::io::Result<sys_mount::Mount> {
        std::fs::create_dir_all("/mnt/squash")?;
        sys_mount::Mount::builder()
            .fstype("squashfs")
            .mount(crate::util::DEFAULT_SQUASH_LOCATION, "/mnt/squash")
    }
    fn cfgdir(&self) -> PathBuf {
        match self {
            Self::ChromebookInstall => const_format::concatcp!(REPART_DIR, "chromebookinstall"),
            _ => todo!(),
        }
        .into()
    }
    #[tracing::instrument]
    fn systemd_repart(blockdev: &Path, cfgdir: &Path) -> Result<()> {
        let dry_run = if cfg!(debug_assertions) { "yes" } else { "no" };
        tracing::debug!(?dry_run, "Running systemd-repart");
        cmd_lib::run_cmd!(
            pkexec systemd-repart
                --dry-run=$dry_run
                --definitions=$cfgdir
                --empty=force
                --json=pretty
                $blockdev
        )
        .map_err(|e| color_eyre::eyre::eyre!("systemd-repart failed").wrap_err(e))?;

        // todo: wait for systemd 256 or genfstab magic
        tracing::debug!("systemd-repart finished");
        Ok(())
    }
    fn set_cgpt_flags(blockdev: &Path) -> Result<()> {
        tracing::debug!("Setting cgpt flags");
        cmd_lib::run_cmd!(cgpt add -i 1 -t kernel -P 15 -T 1 -S 1 $blockdev)?;
        Ok(())
    }
}
