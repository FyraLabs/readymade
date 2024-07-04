use color_eyre::Result;
use std::path::{Path, PathBuf};
use sys_mount::Unmount;

use crate::util::LIVE_BASE;

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

    fn mount_live_base() -> std::io::Result<sys_mount::Mount> {
        const MOUNTPOINT: &str = "/mnt/live-base";
        std::fs::create_dir_all(MOUNTPOINT)?;
        sys_mount::Mount::builder().mount(crate::util::LIVE_BASE, MOUNTPOINT)
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
        let copy_source = {
            const FALLBACK: &str = "/mnt/live-base";
            // We'll be using a new feature from systemd 255 (relative repart copy source)
            // to copy the repartitioning definitions from the live base to the target disk

            // environment variable override. This is documented in HACKING.md

            if let Ok(copy_source) = std::env::var("REPART_COPY_SOURCE") {
                tracing::info!("Using REPART_COPY_SOURCE override: {}", copy_source);
                let copy_source = Path::new(&copy_source.trim()).canonicalize()?;

                if copy_source == Path::new("/") {
                    tracing::warn!("REPART_COPY_SOURCE is set to `/`, this is likely a mistake. Copying entire host root filesystem to target disk...");
                }

                // convert back to string, may cause performance issues but it's not a big deal
                copy_source.to_string_lossy().to_string()
            }
            // if we can mount /dev/mapper/live-base, we'll use that as the copy source
            else {
                match Self::mount_live_base() {
                    Ok(mount) => {
                        let m = mount.target_path().to_string_lossy().to_string();
                        tracing::info!("Mounted live-base at {}", m);
                        m
                    }
                    Err(e) => {
                        tracing::error!("Failed to mount `{LIVE_BASE}`, using `{FALLBACK}` as copy source anyway... ({e})");
                        FALLBACK.to_string()
                    }
                }
            }
        };
        let dry_run = if cfg!(debug_assertions) { "yes" } else { "no" };
        tracing::debug!(?dry_run, "Running systemd-repart");
        cmd_lib::run_cmd!(
            pkexec systemd-repart
                --dry-run=$dry_run
                --definitions=$cfgdir
                --empty=force
                --copy-source=$copy_source
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
