use crate::{
    backend::provisioners::filesystem::FileSystemProvisionerModule, backend::util::fs::copy_dir,
    prelude::*,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Copy {
    /// Either a local path, a mountable image file, or an OCI image reference with
    /// a Podman-recognized transport prefix such as `containers-storage:`.
    pub copy_source: String,
}

const OCI_COPY_SOURCE_PREFIXES: [&str; 4] =
    ["containers-storage:", "docker://", "oci:", "oci-archive:"];

fn is_oci_copy_source(copy_source: &str) -> bool {
    OCI_COPY_SOURCE_PREFIXES
        .iter()
        .any(|prefix| copy_source.starts_with(prefix))
}

#[tracing::instrument]
fn podman_stdout(args: &[&str], action: &str) -> Result<String> {
    tracing::trace!(?args, "running `podman {action}`");
    let mut cmd = Command::new("podman");
    cmd.args(args);

    // hack: do this to allow tracing to see the full command string
    tracing::trace!(?cmd, "executing command");
    let output = cmd
        .output()
        .wrap_err_with(|| format!("failed to run `podman {action}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        bail!("`podman {action}` failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if stdout.is_empty() {
        bail!("`podman {action}` returned empty stdout");
    }

    Ok(stdout)
}

fn cleanup_podman_container(container_id: &str) {
    if let Err(err) = Command::new("podman")
        .args(["umount", container_id])
        .status()
    {
        tracing::warn!(
            ?err,
            ?container_id,
            "podman umount cleanup command failed to execute"
        );
    }

    if let Err(err) = Command::new("podman")
        .args(["rm", "-f", container_id])
        .status()
    {
        tracing::warn!(
            ?err,
            ?container_id,
            "podman rm cleanup command failed to execute"
        );
    }
}

impl FileSystemProvisionerModule for Copy {
    fn run(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()> {
        let destroot = Path::new("/mnt/custom");
        let mut mounts = mounts.clone();
        mounts.sort_mounts();
        mounts.mount_all(
            destroot,
            playbook.encryption.as_ref().map(|e| &*e.encryption_key),
        )?;

        scopeguard::defer! {
            if let Err(e) = mounts.umount_all(destroot) {
                tracing::error!("Cannot unmount partitions: {e:?}");
            }
        };

        let copy_source = self.copy_source.trim();
        if copy_source.is_empty() {
            bail!("copy_source cannot be empty");
        }

        tracing::trace!(?copy_source, ?destroot);
        if is_oci_copy_source(copy_source) {
            tracing::debug!("Copy source is an OCI reference");
            crate::stage!(extracting "Extracting files" {
                tracing::info!(copy_source, "Copy source is an OCI image, mounting with podman");
                let container_id = podman_stdout(&["create", copy_source], "create")?;
                scopeguard::defer! {
                    cleanup_podman_container(&container_id);
                }

                let mount_path = podman_stdout(&["mount", &container_id], "mount")?;
                copy_dir(&mount_path, destroot)?;
            });
        } else {
            let copy_source = PathBuf::from(copy_source);
            if copy_source.is_file() {
                // XXX: we should be using consistent paths, maybe a const? -ci
                const MOUNT_PATH: &str = "/mnt/rdmsqsh";
                tracing::warn!("Copy source is a file, treating as an image to mount");
                crate::stage!(extracting "Extracting files" {
                    tracing::trace!(?MOUNT_PATH, "Mounting disk image");
                    std::fs::create_dir_all(MOUNT_PATH)?;
                    let return_code = Command::new("mount")
                        .arg(&copy_source)
                        .arg(MOUNT_PATH)
                        .status()?
                        .code();
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
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::is_oci_copy_source;

    #[test]
    fn detects_oci_copy_sources() {
        assert!(is_oci_copy_source(
            "containers-storage:registry.example.org/example/os:latest"
        ));
        assert!(is_oci_copy_source("docker://quay.io/fyralabs/os:latest"));
    }

    #[test]
    fn ignores_plain_paths() {
        assert!(!is_oci_copy_source("/mnt/install-root"));
        assert!(!is_oci_copy_source("./install.img"));
    }
}
