use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

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
pub fn install_custom(mounttags: &mut MountTargets) -> color_eyre::Result<()> {
    let destroot = Path::new("/mnt/custom");
    mounttags.sort_mounts();
    mounttags.mount_all(destroot)?;

    let copy_source =
        std::path::PathBuf::from(crate::install::InstallationState::determine_copy_source());
    if copy_source.is_file() {
        // TODO: impl callback status progress
        super::mksys::unsquash_copy(&copy_source, destroot, |_, _| {})?;
    } else {
        todo!("impl copy from dir!");
    }

    mounttags.umount_all(destroot)?;

    let temp_dir = tempfile::tempdir()?.into_path();

    let mut container = tiffin::Container::new(temp_dir);

    for mnt in &mounttags.0 {
        container.add_mount(
            tiffin::MountTarget {
                target: mnt.mountpoint.clone(),
                flags: sys_mount::MountFlags::empty(),
                data: None,
                fstype: None,
            },
            mnt.partition.clone(),
        );
    }

    let uefi = crate::util::check_uefi();
    container.run(|| crate::install::_inner_sys_setup(uefi))??;

    Ok(())
}
