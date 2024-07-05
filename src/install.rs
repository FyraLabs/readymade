use color_eyre::{eyre::eyre, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    backend::repart_output::{systemd_version, RepartOutput},
    util::{self, LIVE_BASE},
};

const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";

#[derive(Debug, Clone)]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}


#[tracing::instrument]
pub fn setup_system(output: RepartOutput) -> Result<()> {
    let mut container = output.to_container()?;

    // note: that nesting is crazy bruh
    // todo: cleanup
    
    // The reason we're checking for UEFI here is because we want to check the current
    // system's boot mode before we install GRUB, not check inside the container
    let uefi = util::check_uefi();
    container
        .run(|| {
            if uefi {
                // The reason why we don't do grub2-install here is because for
                // Fedora specifically, the install script simply plops in
                // a pre-built GRUB binary in the ESP that looks for the stage 1
                // config in /boot/efi/EFI/fedora/grub.cfg
                // The following config then redirects to the actual stage 2 config located
                // in /boot/grub2/grub.cfg
                // This is actually done to support BLS entries properly on their end

                // todo: Add support for systemd-boot
                std::fs::create_dir_all("/boot/efi/EFI/fedora")?;

                let s = tracing::info_span!("Generating stage 1 grub.cfg in ESP...");
                {
                    let _guard = s.enter();
                    let mut grub_cfg = std::fs::File::create("/boot/efi/EFI/fedora/grub.cfg")?;
                    grub_cfg.write_all(crate::util::grub_config().as_bytes())?;
                }

                let s = tracing::info_span!("Generating stage 2 grub.cfg in /boot/grub2/grub.cfg...");
                {
                    let _guard = s.enter();
                    let _ = std::process::Command::new("grub2-mkconfig")
                        .arg("-o")
                        .arg("/boot/grub2/grub.cfg")
                        .status()?;
                }
            }

            // Clean up /boot partition
            {
                // tracing::info!("Cleaning up /boot partition...");
                let span = tracing::info_span!("Cleaning up /boot partition...");
                let _guard = span.enter();
                // Clean up initramfs and vmlinuz
                {
                    let boot_dir = Path::new("/boot");
                    let boot_files = std::fs::read_dir(boot_dir)?
                        .map(|entry| entry.unwrap().path())
                        .collect::<Vec<_>>();

                    for file in boot_files {
                        let file_name = file.file_name().unwrap().to_str().unwrap();
                        if file_name.starts_with("initramfs")
                            || file_name.starts_with("vmlinuz")
                        {
                            tracing::debug!(?file, "Removing kernel file");
                            std::fs::remove_file(file)?;
                        }
                    }
                }

                // clean up old BLS entries
                {
                    let bls_dir = Path::new("/boot/loader/entries");
                    let bls_files = std::fs::read_dir(bls_dir)?
                        .map(|entry| entry.unwrap().path())
                        .collect::<Vec<_>>();

                    for file in bls_files {
                        tracing::debug!(?file, "Removing BLS entry");
                        std::fs::remove_file(file)?;
                    }
                }
                drop(_guard);
            }

            // Reinstall kernel
            // 
            // Here we're going to reinstall the kernel with an initramfs optimized
            // for the new system configuration. We'll be doing this by using kernel-install
            // 
            // which runs all the necessary hooks to generate the initramfs and install the kernel properly.
            // 
            // As a bonus, it also generates the BLS entries for us.
            {
                tracing::info!("Reinstalling kernel...");
                // list all kernels in /lib/modules
                // suggestion: Switch to using kernel-install --json=short for parsing
                let kernel_vers = std::fs::read_dir("/lib/modules")?
                    .map(|entry| entry.unwrap().file_name())
                    .collect::<Vec<_>>();

                tracing::info!(?kernel_vers, "Kernel versions found");
                
                // We're gonna just install the first kernel we find, so let's do that
                let kver = kernel_vers.iter().next().unwrap().to_str().unwrap();
                
                // install kernel
                
                std::process::Command::new("kernel-install")
                    .arg("add")
                    .arg(kver)
                    .arg(format!("/lib/modules/{kver}/vmlinuz"))
                    .arg(format!("--verbose"))
                    .status()?;
            }
            
            
            // Generate /etc/fstab
            if systemd_version()? <= 256 {
                tracing::info!("Generating /etc/fstab...");
                let mut fstab = std::fs::File::create("/etc/fstab")?;
                fstab.write_all(output.into_fstab().as_bytes())?;
            }
            
            // todo: restore selinux contexts

            Ok(())
        })
        .map_err(|e| eyre!("Error configuring system: {}", e))?;

    Ok(())
}

impl InstallationType {


    #[tracing::instrument]
    pub fn install(&self, state: &crate::InstallationState) -> Result<()> {
        let blockdev = &state.destination_disk.as_ref().unwrap().devpath;
        let cfgdir = self.cfgdir();

        let repart_out = Self::systemd_repart(blockdev, &cfgdir)?;
        tracing::info!("Copying files done, Setting up system...");
        setup_system(repart_out)?;

        if let Self::ChromebookInstall = self {
            // todo: not freeze on error, show error message as err handler?
            Self::set_cgpt_flags(blockdev)?;
        }
        tracing::info!("install() finished");
        Ok(())
    }
    // fn mount_squashimg() -> std::io::Result<sys_mount::Mount> {
    //     std::fs::create_dir_all("/mnt/squash")?;
    //     sys_mount::Mount::builder()
    //         .fstype("squashfs")
    //         .mount(crate::util::DEFAULT_SQUASH_LOCATION, "/mnt/squash")
    // }

    /// Mount a device or file to /mnt/live-base
    fn mount_dev(dev: &str) -> std::io::Result<sys_mount::Mount> {
        const MOUNTPOINT: &str = "/mnt/live-base";
        std::fs::create_dir_all(MOUNTPOINT)?;
        sys_mount::Mount::builder().mount(dev, MOUNTPOINT)
    }
    fn cfgdir(&self) -> PathBuf {
        match self {
            Self::ChromebookInstall => const_format::concatcp!(REPART_DIR, "chromebookinstall"),
            _ => todo!(),
        }
        .into()
    }
    #[tracing::instrument]
    fn systemd_repart(
        blockdev: &Path,
        cfgdir: &Path,
    ) -> Result<crate::backend::repart_output::RepartOutput> {
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
                match Self::mount_dev(crate::util::LIVE_BASE) {
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
        let out = cmd_lib::run_fun!(
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
        Ok(serde_json::from_str(&out)?)
    }
    fn set_cgpt_flags(blockdev: &Path) -> Result<()> {
        tracing::debug!("Setting cgpt flags");
        cmd_lib::run_cmd!(cgpt add -i 1 -t kernel -P 15 -T 1 -S 1 $blockdev)?;
        Ok(())
    }
}
