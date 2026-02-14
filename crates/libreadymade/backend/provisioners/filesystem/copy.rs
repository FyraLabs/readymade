use crate::{
    backend::provisioners::{Mount, Mounts, filesystem::FileSystemProvisionerModule},
    prelude::*,
    util::fs::copy_dir,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Copy {
    pub copy_source: PathBuf,
}

impl FileSystemProvisionerModule for Copy {
    fn run(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()> {
        let destroot = Path::new("/mnt/custom");
        let mut mounts = mounts.clone();
        mounts.sort_mounts();
        mounts.mount_all(destroot, playbook.encryption.as_ref().map(|e| &*e.encryption_key))?;

        scopeguard::defer! {
            if let Err(e) = mounts.umount_all(destroot) {
                tracing::error!("Cannot unmount partitions: {e:?}");
            }
        };

        let copy_source = &self.copy_source;
        tracing::trace!(?copy_source, ?destroot);
        if copy_source.is_file() {
            // XXX: we should be using consistent paths, maybe a const? -ci
            const MOUNT_PATH: &str = "/mnt/rdmsqsh";
            tracing::warn!("Copy source is a file, treating as an image to mount");
            crate::stage!(extracting "Extracting files" {
                tracing::trace!(?MOUNT_PATH, "Mounting disk image");
                let return_code = Command::new("mount").arg(copy_source).arg(MOUNT_PATH).status()?.code();
                if return_code.is_none_or(|return_code| return_code != 0) {
                    bail!("mount command returns rc={return_code:?}");
                }
                scopeguard::defer! {
                    _ = Command::new("umount").arg(MOUNT_PATH).status();
                }
                copy_dir(MOUNT_PATH, destroot)?;
            });
        } else {
            crate::stage!(copying "Copying files" {
                copy_dir(&copy_source, destroot)?;
            });
        }

        Ok(())
    }
}
