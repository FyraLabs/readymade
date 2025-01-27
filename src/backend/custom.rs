use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use color_eyre::eyre::Context;

use super::install::InstallationState;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MountTarget {
    #[doc(hidden)]
    pub index: usize,
    pub partition: PathBuf,
    pub mountpoint: PathBuf,
    pub options: String,
}

impl MountTarget {
    fn mount(&self, root: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(root)?;
        let target = (self.mountpoint.strip_prefix("/")).unwrap_or(&self.mountpoint);
        tracing::info!(?root, "Mounting {:?} to {target:?}", self.partition);
        let target = root.join(target);
        std::fs::create_dir_all(&target)?;

        sys_mount::Mount::builder()
            .data(&self.options)
            .mount(&self.partition, target)?;
        Ok(())
    }

    pub fn umount(&self, root: &Path) -> std::io::Result<()> {
        // sanitize target path
        let target = (self.mountpoint.strip_prefix("/")).unwrap_or(&self.mountpoint);
        let target = root.join(target);

        nix::mount::umount(&target)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MountTargets(pub Vec<MountTarget>);

impl MountTargets {
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
    fn mount_all(&self, root: &Path) -> std::io::Result<()> {
        self.0.iter().try_for_each(|m| m.mount(root))
    }

    /// Unmount all the targets in reverse.
    fn umount_all(&self, root: &Path) -> std::io::Result<()> {
        self.0.iter().rev().try_for_each(|m| m.umount(root))
    }
}

// 1. mount all
// 2. copy stuff
// 3. funny setup_system()
#[allow(clippy::cognitive_complexity, clippy::module_name_repetitions)]
pub fn install_custom(
    state: &InstallationState,
    mounttags: &mut MountTargets,
) -> color_eyre::Result<()> {
    let destroot = Path::new("/mnt/custom");
    mounttags.sort_mounts();
    mounttags.mount_all(destroot)?;

    {
        scopeguard::defer! {
            if let Err(e) = mounttags.umount_all(destroot) {
                tracing::error!("Cannot unmount partitions: {e:?}");
            }
        };

        let copy_source = PathBuf::from(InstallationState::determine_copy_source());
        tracing::trace!(?copy_source, ?destroot);
        if copy_source.is_file() {
            crate::stage!("Extracting files" {
                // super::mksys::unsquash_copy(&copy_source, destroot, |_, _| {})?;
                //
                // FIXME: I give up idk why this doesn't work
                // let mt = MountTarget { partition: copy_source, mountpoint: "/mnt/rdmsqsh".into(),
                //     options: "loop".into(),
                //     ..Default::default() };
                // mt.mount("/".as_ref())?;
                // scopeguard::defer! {
                //     if let Err(e) = mt.umount("/".as_ref()) {
                //         tracing::error!("Cannot unmount /mnt/rdmsqsh: {e:?}");
                //     }
                // };
                // and it says
                // 0: Invalid argument (os error 22)
                // the error is from the nix mount call I think
                let rc = std::process::Command::new("mount").arg(copy_source).arg("/mnt/rdmsqsh").status()?.code();
                if rc.is_none_or(|rc| rc != 0) {
                    color_eyre::eyre::bail!("mount command returns rc={rc:?}");
                }
                scopeguard::defer! {
                    _ = std::process::Command::new("umount").arg("/mnt/rdmsqsh").status();
                }
                crate::util::fs::copy_dir("/mnt/rdmsqsh", destroot)?;
            });
        } else {
            crate::stage!("Copying files" {
                crate::util::fs::copy_dir_cp(&copy_source, destroot)?;
            });
        }
    }

    let temp_dir = tempfile::tempdir()?.into_path();

    let mut container = tiffin::Container::new(temp_dir);

    for MountTarget {
        partition,
        mountpoint,
        ..
    } in mounttags.0.clone()
    {
        container.add_mount(
            tiffin::MountTarget {
                target: mountpoint,
                ..Default::default()
            },
            partition,
        );
    }

    let fstab = filesystem_table::generate_fstab("/mnt/custom/")
        .wrap_err("cannot generate fstab")?
        .to_string();

    let efi = (mounttags.0.iter())
        .find(|part| part.mountpoint == std::path::Path::new("/boot/efi"))
        .and_then(|part| part.partition.to_str().map(ToOwned::to_owned));
    let xbootldr = (mounttags.0.iter())
        .find(|part| part.mountpoint == std::path::Path::new("/boot"))
        .and_then(|part| part.partition.to_str().map(ToOwned::to_owned))
        .ok_or_else(|| color_eyre::eyre::eyre!("cannot find xbootldr partition"))?;

    container.run(|| state._inner_sys_setup(fstab, efi, &xbootldr))??;

    Ok(())
}
