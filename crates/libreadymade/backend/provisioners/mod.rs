use crate::prelude::*;

use std::{
    fmt::Write,
    fs::create_dir_all,
    path::{Component, Path, PathBuf},
};

use color_eyre::eyre::bail;
use nix::mount::umount;

pub mod disk;
pub mod filesystem;

#[derive(Debug, Clone)]
pub enum EncryptionOption {
    // TODO: document
    KeyFile,
    Tpm2,
    KeyFileTpm2,
}

#[derive(Debug, Clone)]
pub struct Mount {
    /// Path to partition
    pub partition: PathBuf,
    /// Path to mountpoint
    pub mountpoint: PathBuf,
    /// Raw text `mountopts`
    pub options: String,
    /// Encryption type of the partition, if any
    pub encryption_type: Option<EncryptionOption>,
    /// Label of the partition
    pub label: Option<String>,
}

impl Mount {
    fn mount(&self, root: &Path, passphrase: Option<&str>) -> std::io::Result<()> {
        create_dir_all(root)?;

        let target = (self.mountpoint.strip_prefix("/")).unwrap_or(&self.mountpoint);

        tracing::info!(?root, "Mounting {:?} to {target:?}", self.partition);

        let target = root.join(target);
        create_dir_all(&target)?;

        if let Some(_) = self.encryption_type {
            let label = crate::backend::repart_output::generate_unique_mapper_label(
                format!("{}", self.mountpoint.display()).as_str(),
            );
            let mapper = crate::backend::repart_output::luks_decrypt(
                format!("{}", self.partition.display()).as_str(),
                passphrase.unwrap(),
                &label,
            )?;
        }

        sys_mount::Mount::builder()
            .data(&self.options)
            .mount(&self.partition, target)?;

        Ok(())
    }

    pub fn umount(&self, root: &Path) -> std::io::Result<()> {
        // sanitize target path
        let target = (self.mountpoint.strip_prefix("/")).unwrap_or(&self.mountpoint);
        let target = root.join(target);

        umount(&target)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Mounts(pub Vec<Mount>);

impl Mounts {
    //? https://github.com/FyraLabs/tiffin/blob/3d09faf3127f644fbd441af78d039b1acaba5847/src/lib.rs#L117C1-L130C6
    /// Sort mounts by mountpoint and depth
    /// Closer to root, and root is first
    /// everything else is either sorted by depth, or alphabetically
    fn sort_mounts(&mut self) {
        self.0.sort_by(|a, b| {
            match (
                a.mountpoint.components().count(),
                b.mountpoint.components().count(),
            ) {
                (1, _) if a.mountpoint.components().next() == Some(Component::RootDir) => {
                    std::cmp::Ordering::Less
                } // root dir
                (_, 1) if b.mountpoint.components().next() == Some(Component::RootDir) => {
                    std::cmp::Ordering::Greater
                } // root dir
                (x, y) if x == y => a.mountpoint.cmp(&b.mountpoint),
                (x, y) => x.cmp(&y),
            }
        });
    }

    /// Mount all the targets in the specified order.
    fn mount_all(&self, root: &Path, passphrase: Option<&str>) -> std::io::Result<()> {
        self.0.iter().try_for_each(|m| m.mount(root, passphrase))
    }

    /// Unmount all the targets in reverse.
    fn umount_all(&self, root: &Path) -> std::io::Result<()> {
        self.0.iter().rev().try_for_each(|m| m.umount(root))
    }
}

/// Wrapper struct for encryption data, so we don't have to pass around multiple
/// arguments
#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone)]
pub struct CryptData {
    /// Contents of /etc/crypttab
    pub crypttab: String,

    /// Extra cmdline options for the kernel
    pub cmdline_opts: Vec<String>,
    pub tpm: bool,
}

#[must_use]
pub fn is_luks(node: &str) -> bool {
    std::process::Command::new("cryptsetup")
        .args(["isLuks", node])
        .status()
        .expect("cannot run cryptsetup")
        .success()
}
fn cryptsetup_luks_uuid(node: &str) -> Result<String, color_eyre::eyre::Error> {
    let cmd = std::process::Command::new("cryptsetup")
        .arg("luksUUID")
        .arg(node)
        .output()?;
    if !cmd.status.success() {
        color_eyre::eyre::bail!(
            "cryptsetup failed: {}",
            String::from_utf8_lossy(&cmd.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&cmd.stdout).trim().to_owned())
}

#[allow(clippy::unwrap_in_result)]
/// # Panics
/// if LUKS UUID cannot be obtained.
pub fn generate_cryptdata(mounts: &Mounts) -> Result<Option<CryptData>, color_eyre::eyre::Error> {
    // NOTE: https://www.man7.org/linux/man-pages/man5/crypttab.5.html
    let mut crypttab = String::new();
    let mut cmdline_opts = vec![];

    let luks_partitions = mounts.0.iter().filter(|part| is_luks(&part.partition));

    let mut is_tpm = false;

    let has_luks = luks_partitions.clone().count() > 0;
    for part in luks_partitions {
        let uuid = cryptsetup_luks_uuid(&part.node).expect("Failed to get LUKS UUID");
        let label = &part.label;

        let mut extra_opts = String::new();

        let part_uses_tpm: bool = {
            matches!(
                part.encryption_type,
                super::repartcfg::EncryptOption::KeyFileTpm2
                    | super::repartcfg::EncryptOption::Tpm2
            )
        };

        if part_uses_tpm {
            is_tpm = true;
            extra_opts.push_str("tpm2-device=auto,");
        }

        writeln!(
            &mut crypttab,
            "{label}\tUUID={uuid}\tnone\t{extra_opts}luks,discard"
        )?;

        cmdline_opts.push(format!("rd.luks.name={uuid}={label}"));
    }

    if is_tpm {
        cmdline_opts.push("rd.luks.options=tpm2-device=auto".to_owned());
    }

    Ok(has_luks.then_some(CryptData {
        crypttab,
        cmdline_opts,
        tpm: is_tpm,
    }))
}
