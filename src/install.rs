use color_eyre::eyre::eyre;
use color_eyre::{Result, Section};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crate::util::{exist_then, exist_then_read_dir};
use crate::{
    backend::repart_output::{systemd_version, RepartOutput},
    pages::destination::DiskInit,
    stage,
    util::{self, LIVE_BASE},
};

const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";

#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallationState {
    pub langlocale: Option<String>,
    pub destination_disk: Option<DiskInit>,
    pub installation_type: Option<InstallationType>,
}

// TODO: remove this after have support for anything other than chromebook
impl Default for InstallationState {
    fn default() -> Self {
        Self {
            langlocale: Default::default(),
            destination_disk: Default::default(),
            installation_type: if let [one] = &crate::CONFIG.read().install.allowed_installtypes[..]
            {
                Some(*one)
            } else {
                None
            },
        }
    }
}

impl InstallationState {
    // todo: move methods from installationstate to here!
    pub fn install_using_subprocess(&self) -> Result<()> {
        let mut command = Command::new("pkexec");
        command
            .arg(std::env::current_exe()?)
            .arg("--non-interactive")
            .stdin(std::process::Stdio::piped());

        // pass in REPART_COPY_SOURCE if it's set
        // This is a bit hacky, I should fix this later
        if std::env::var("REPART_COPY_SOURCE").is_ok() {
            command.env("REPART_COPY_SOURCE", std::env::var("REPART_COPY_SOURCE")?);
        }

        print!("┌─ BEGIN: Readymade subprocess logs\n│ ");
        let res = crate::util::cmds::read_while_show_output(&mut command, Some("│ "), |hdl| {
            let mut child_stdin = hdl.stdin.take().expect("can't take stdin");
            child_stdin.write_all(serde_json::to_string(self)?.as_bytes())?;
            hdl.stdin = Some(child_stdin);
            Ok(())
        });
        println!("└─ END OF Readymade subprocess logs");

        match res {
            Ok((exit_status, ..)) if exit_status.success() => Ok(()),
            Ok((exit_status, stdout, stderr)) => Err(eyre!("Readymade subprocess failed")
                .with_note(|| exit_status.to_string())
                .with_note(|| format!("Stdout:\n{}", strip_ansi_escapes::strip_str(&stdout)))
                .with_note(|| format!("Stderr:\n{}", strip_ansi_escapes::strip_str(&stderr)))),
            Err(e) => Err(eyre!("Failed to execute readymade non-interactively").wrap_err(e)),
        }
    }

    #[allow(clippy::unwrap_in_result)]
    #[tracing::instrument]
    pub fn install(&self) -> Result<()> {
        let inst_type = self
            .installation_type
            .as_ref()
            .expect("A valid installation type should be set before calling install()");
        let blockdev = &self
            .destination_disk
            .as_ref()
            .expect("A valid destination device should be set before calling install()")
            .devpath;
        let cfgdir = inst_type.cfgdir();
        let repart_out = stage!("Creating partitions and copying files" {
            // todo: not freeze on error, show error message as err handler?
            Self::systemd_repart(blockdev, &cfgdir)?
        });

        tracing::info!("Copying files done, Setting up system...");
        setup_system(repart_out)?;

        if let InstallationType::ChromebookInstall = inst_type {
            InstallationType::set_cgpt_flags(blockdev)?;
        }
        tracing::info!("install() finished");
        Ok(())
    }

    /// Mount a device or file to /mnt/live-base
    fn mount_dev(dev: &str) -> std::io::Result<sys_mount::Mount> {
        const MOUNTPOINT: &str = "/mnt/live-base";
        std::fs::create_dir_all(MOUNTPOINT)?;
        sys_mount::Mount::builder().mount(dev, MOUNTPOINT)
    }

    // todo: Generate custom repart partitioning definitions in case the user wants to use a custom partitioning scheme
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
                tracing::info!("Using REPART_COPY_SOURCE override: {copy_source}");
                let copy_source = Path::new(&copy_source.trim()).canonicalize()?;

                if copy_source == Path::new("/") {
                    tracing::warn!("REPART_COPY_SOURCE is set to `/`, this is likely a mistake. Copying entire host root filesystem to target disk...");
                }

                // convert back to string, may cause performance issues but it's not a big deal
                copy_source.to_string_lossy().to_string()
            }
            // if /run/rootfsbase exists and is a directory, we'll use that as the copy source
            else if std::fs::metadata(crate::util::ROOTFS_BASE)
                .map(|m| m.is_dir())
                .unwrap_or(false)
            {
                tracing::info!(
                    "Using {} as copy source, as it exists presumably due to raw rootfs in dracut",
                    crate::util::ROOTFS_BASE
                );
                crate::util::ROOTFS_BASE.to_owned()
            }
            // if we can mount /dev/mapper/live-base, we'll use that as the copy source
            else {
                match Self::mount_dev(crate::util::LIVE_BASE) {
                    Ok(mount) => {
                        let m = mount.target_path().to_string_lossy().to_string();
                        tracing::info!("Mounted live-base at {m}");
                        m
                    }
                    Err(e) => {
                        tracing::error!("Failed to mount `{LIVE_BASE}`, using `{FALLBACK}` as copy source anyway... ({e})");
                        FALLBACK.to_owned()
                    }
                }
            }
        };

        let arg = if systemd_version()? >= 256 {
            "--generate-fstab"
        } else {
            ""
        };

        let dry_run = if cfg!(debug_assertions) { "yes" } else { "no" };
        tracing::debug!(?dry_run, "Running systemd-repart");
        let out = cmd_lib::run_fun!(
            pkexec systemd-repart
                --dry-run=$dry_run
                --definitions=$cfgdir
                --empty=force
                --offline=false
                $arg
                --copy-source=$copy_source
                --json=pretty
                $blockdev
        )
        .map_err(|e| color_eyre::eyre::eyre!("systemd-repart failed").wrap_err(e))?;

        // todo: wait for systemd 256 or genfstab magic
        tracing::debug!(out, "systemd-repart finished");
        Ok(serde_json::from_str(&out)?)
    }
}

#[tracing::instrument]
pub fn setup_system(output: RepartOutput) -> Result<()> {
    let mut container = output.to_container()?;

    // The reason we're checking for UEFI here is because we want to check the current
    // system's boot mode before we install GRUB, not check inside the container
    let uefi = util::check_uefi();
    container.run(|| _inner_sys_setup(uefi, output))?
}

#[tracing::instrument]
fn _inner_sys_setup(uefi: bool, output: RepartOutput) -> Result<()> {
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

        stage!("Generating stage 1 grub.cfg in ESP..." {
            let mut grub_cfg = std::fs::File::create("/boot/efi/EFI/fedora/grub.cfg")?;
            // let's search for an xbootldr label
            // because we never know what the device will be
            // see the compile time included config file
            grub_cfg.write_all(include_bytes!("../templates/fedora-grub.cfg"))?;
        });

        stage!("Generating stage 2 grub.cfg in /boot/grub2/grub.cfg..." {
            _ = std::process::Command::new("grub2-mkconfig")
                .arg("-o")
                .arg("/boot/grub2/grub.cfg")
                .status()?;
        });
    }

    stage!("Cleaning up /boot partition..." {
        for file in std::fs::read_dir("/boot")?.flatten().map(|entry| entry.path()) {
            let file_name = file.file_name().unwrap().to_str().unwrap();
            if file_name.starts_with("initramfs") || file_name.starts_with("vmlinuz") {
                tracing::debug!(?file, "Removing kernel file");
                std::fs::remove_file(file)?;
            }
        }

        for file in std::fs::read_dir("/boot/loader/entries")?.flatten().map(|entry| entry.path()) {
            tracing::debug!(?file, "Removing BLS entry");
            std::fs::remove_file(file)?;
        }
    });

    // Reinstall kernel
    //
    // Here we're going to reinstall the kernel with an initramfs optimized
    // for the new system configuration. We'll be doing this by using kernel-install
    //
    // which runs all the necessary hooks to generate the initramfs and install the kernel properly.
    //
    // As a bonus, it also generates the BLS entries for us.
    stage!("Reinstalling kernel" {
        // list all kernels in /lib/modules
        // suggestion: Switch to using kernel-install --json=short for parsing
        let kernel_vers = std::fs::read_dir("/lib/modules")?
            .map(|entry| entry.unwrap().file_name())
            .collect_vec();

        tracing::info!(?kernel_vers, "Kernel versions found");

        // We're gonna just install the first kernel we find, so let's do that
        let kver = kernel_vers.first().unwrap().to_str().unwrap();

        // install kernel

        std::process::Command::new("kernel-install")
            .arg("add")
            .arg(kver)
            .arg(format!("/lib/modules/{kver}/vmlinuz"))
            .arg("--verbose")
            .status()?;
    });
    if systemd_version()? <= 256 {
        stage!("Generating /etc/fstab..." {
            let mut fstab = std::fs::File::create("/etc/fstab")?;
            fstab.write_all(output.generate_fstab().as_bytes())?;
        });
    }

    stage!("Regenerating initramfs" {
        // We assume the installation wouldn't be used on another system (false only if you install
        // on something like a USB stick anyway)
        // → reduce size of initramfs aggressively for faster boot times
        //
        // on my system this reduces the size from 170M down to 43M.
        // — mado
        std::process::Command::new("dracut").args([
            "--force",
            "--parallel",
            "--regenerate-all",
            "--hostonly",
            "--strip",
            "--aggressive-strip",
        ]).status()?;
    });

    stage!("Initializing system" {
        _initialize_system()?;
    });

    stage!("Setting SELinux contexts..." {
        std::process::Command::new("setfiles")
            .args(["-e", "/proc", "-e", "/sys"])
            .arg("/etc/selinux/targeted/contexts/files/file_contexts")
            .arg("/")
            .status()?;
    });

    Ok(())
}

/// Initialize the system after installation
/// This function is moved to a separate function to allow for cleaner code
#[tracing::instrument]
fn _initialize_system() -> color_eyre::Result<()> {
    exist_then(std::fs::remove_file("/var/lib/systemd/random-seed"))?;
    // We're gonna make an empty machine-id file so that systemd can generate a new one
    std::fs::File::create("/etc/machine-id")?;

    // wipe NetworkManager state
    exist_then(std::fs::remove_dir_all(
        "/etc/NetworkManager/system-connections",
    ))?;
    std::fs::create_dir_all("/etc/NetworkManager/system-connections")?;

    // todo: Copy over NetworkManager state from current livesys

    // wipe temporary RPM database
    exist_then_read_dir("/var/lib/rpm")?
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("__db"))
        .map(|entry| entry.path())
        .try_for_each(std::fs::remove_file)?;

    // wipe temporary dnf cache
    exist_then(std::fs::remove_dir_all("/var/cache/dnf"))?;

    // todo: set locale and timezone from config

    Ok(())
}

impl InstallationType {
    fn cfgdir(&self) -> PathBuf {
        match self {
            Self::ChromebookInstall => const_format::concatcp!(REPART_DIR, "chromebookinstall"),
            _ => todo!(),
        }
        .into()
    }

    fn set_cgpt_flags(blockdev: &Path) -> Result<()> {
        tracing::debug!("Setting cgpt flags");
        cmd_lib::run_cmd!(cgpt add -i 1 -t kernel -P 15 -T 1 -S 1 $blockdev)?;
        Ok(())
    }
}
