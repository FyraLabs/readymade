use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use color_eyre::eyre::Context;

use super::install::InstallationState;

const DIR_COPY_OPTS: fs_more::directory::DirectoryCopyOptions = {
    use fs_more::directory::{
        BrokenSymlinkBehaviour, CollidingSubDirectoryBehaviour, DestinationDirectoryRule,
        DirectoryCopyDepthLimit, DirectoryCopyOptions, SymlinkBehaviour,
    };
    use fs_more::file::CollidingFileBehaviour;
    DirectoryCopyOptions {
        destination_directory_rule: DestinationDirectoryRule::AllowNonEmpty {
            colliding_file_behaviour: CollidingFileBehaviour::Overwrite,
            colliding_subdirectory_behaviour: CollidingSubDirectoryBehaviour::Continue,
        },
        copy_depth_limit: DirectoryCopyDepthLimit::Unlimited,
        symlink_behaviour: SymlinkBehaviour::Keep,
        broken_symlink_behaviour: BrokenSymlinkBehaviour::Keep,
    }
};

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
        let target = self
            .mountpoint
            .strip_prefix("/")
            .unwrap_or(&self.mountpoint);
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
        let target = self
            .mountpoint
            .strip_prefix("/")
            .unwrap_or(&self.mountpoint);
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
        if copy_source.is_file() {
            crate::stage!("Extracting files" {
                super::mksys::unsquash_copy(&copy_source, destroot, |_, _| {})?;
            });
        } else {
            crate::stage!("Copying files" {
                fs_more::directory::copy_directory(&copy_source, destroot, DIR_COPY_OPTS).wrap_err("can't copy files")?;
            });
            tracing::info!(?copy_source, ?destroot, "Copying directory");
            // crate::util::fs::copy_dir(&copy_source, destroot)?;
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

    let mut fstab = std::process::Command::new("mkfstab");
    let fstab = fstab.arg("/mnt/custom/"); // root
    let fstab = fstab.stdout(std::process::Stdio::piped());
    let fstab = fstab.output()?.stdout;
    let fstab = String::from_utf8(fstab)?;

    let efi = (mounttags.0.iter())
        .find(|part| part.mountpoint == std::path::Path::new("/boot/efi"))
        .and_then(|part| part.partition.to_str().map(ToOwned::to_owned));

    container.run(|| state._inner_sys_setup(fstab, efi))??;

    Ok(())
}
