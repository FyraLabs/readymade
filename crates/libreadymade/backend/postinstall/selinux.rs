//! Labels the installed system with its own `SELinux` policy.
//!
//! - Files copied out of a container image carry podman's `container_file_t` label, which the
//!   policy marks customizable, so setfiles only replaces it with `-F`.
//! - setfiles doesn't cross mounts, and every btrfs subvolume counts as one. Each filesystem of
//!   the target (its mounts, and subvolumes like snapper's `/.snapshots` that aren't mounted) is
//!   relabeled through its own non-recursive bind mount. That view shows only that filesystem,
//!   including the mountpoint directories other mounts hide, and none of the host's `/dev`,
//!   `/proc` or `/run/host` the install chroot mounts on top.
//! - The kernel only accepts labels its loaded policy knows, and the installer's policy can lack
//!   types the target's has. Like `bootc install`, setfiles runs as `install_t`, which may write
//!   them anyway; they stay unmapped until the target boots with its own policy.
//!
//! A dry run afterwards checks nothing is left to relabel, so a relabel that quietly did nothing
//! fails the install instead of leaving a system that can't boot enforcing.

use std::{cell::Cell, os::unix::fs::MetadataExt, rc::Rc};

use crate::{backend::util::sys::mount_fstype, prelude::*};
use nix::mount::{MntFlags, MsFlags};
use serde::{Deserialize, Serialize};

use crate::stage;

use super::{Context, PostInstallModule};

const FILE_CONTEXTS: &str = "/etc/selinux/targeted/contexts/files/file_contexts";
const SETFILES: &str = "/usr/sbin/setfiles";
/// Holds the copy of setfiles that runs as `install_t`. It must not be on a nosuid mount, which
/// blocks the domain transition, and the copy must be named `setfiles`: it picks its mode from
/// its file name.
const SETFILES_COPY_DIR: &str = "/var/tmp/readymade-setfiles";
/// Labeled on the setfiles copy so the installer's policy runs it as `install_t`.
const INSTALL_EXEC_CONTEXT: &[u8] = b"system_u:object_r:install_exec_t:s0\0";
/// Filesystems without xattrs, labeled by their mount options instead.
const UNLABELED_FSTYPES: [&str; 5] = ["vfat", "msdos", "exfat", "ntfs3", "fuseblk"];
/// Inode number of the root directory of every btrfs subvolume.
const BTRFS_SUBVOLUME_INO: u64 = 256;
/// How many mislabeled files to list when the check fails.
const MAX_REPORTED: usize = 10;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SELinux;

impl PostInstallModule for SELinux {
    fn name(&self) -> &'static str {
        "SELinux"
    }

    fn run(&self, context: &Context) -> Result<()> {
        let mounts = std::fs::read_to_string("/proc/self/mounts")
            .wrap_err("cannot read /proc/self/mounts")?;
        let mut targets = relabel_targets(
            context.mounts.0.iter().map(|m| m.mountpoint.as_path()),
            &mounts,
        );
        targets.extend(unmounted_subvolumes(&targets, &mounts)?);
        tracing::info!(?targets, "Relabeling filesystems");

        stage!(selinux "Setting SELinux labels" {
            let setfiles = install_t_setfiles()?;
            scopeguard::defer! {
                _ = std::fs::remove_dir_all(SETFILES_COPY_DIR);
            }
            let views = Views::new(&targets)?;

            for view in &views.list {
                let status = Command::new(&setfiles)
                    .args(view.setfiles_args(false))
                    .status()
                    .wrap_err("cannot run setfiles")?;
                if !status.success() {
                    bail!(
                        "setfiles failed on {} with exit code {:?}",
                        view.target,
                        status.code()
                    );
                }
            }

            let mut mislabeled = vec![];
            for view in &views.list {
                let check = Command::new(&setfiles)
                    .args(view.setfiles_args(true))
                    .output()
                    .wrap_err("cannot run setfiles")?;
                if !check.status.success() {
                    bail!(
                        "setfiles could not check labels on {} (exit code {:?})",
                        view.target,
                        check.status.code()
                    );
                }
                mislabeled.extend(
                    String::from_utf8_lossy(&check.stdout)
                        .lines()
                        .filter(|line| line.starts_with("Would relabel"))
                        .map(ToOwned::to_owned),
                );
            }
            if !mislabeled.is_empty() {
                bail!(
                    "{} files still have the wrong SELinux label after relabeling:\n{}",
                    mislabeled.len(),
                    mislabeled.iter().take(MAX_REPORTED).join("\n"),
                );
            }
        });

        Ok(())
    }
}

/// Mountpoints of the target to relabel: `/` first, then every mount on a filesystem that
/// stores labels.
fn relabel_targets<'a>(mountpoints: impl Iterator<Item = &'a Path>, mounts: &str) -> Vec<String> {
    let mut targets = vec!["/".to_owned()];
    for mountpoint in mountpoints {
        let mountpoint = mountpoint.to_string_lossy();
        if targets.iter().any(|t| *t == mountpoint) {
            continue;
        }
        match mount_fstype(mounts, &mountpoint) {
            Some(fstype) if UNLABELED_FSTYPES.contains(&fstype) => {}
            Some(_) => targets.push(mountpoint.into_owned()),
            None => tracing::warn!(%mountpoint, "Not mounted, skipping relabel"),
        }
    }
    targets
}

/// btrfs subvolumes directly inside a btrfs target that aren't mounted anywhere, such as the
/// `/.snapshots` subvolume snapper creates. setfiles stops at them like at any other mount.
fn unmounted_subvolumes(targets: &[String], mounts: &str) -> Result<Vec<String>> {
    let mut found = vec![];
    for target in targets {
        if mount_fstype(mounts, target) != Some("btrfs") {
            continue;
        }
        let parent_dev = std::fs::metadata(target)?.dev();
        for entry in std::fs::read_dir(target)? {
            let path = entry?.path();
            let Ok(meta) = path.symlink_metadata() else {
                continue;
            };
            let path = path.to_string_lossy().into_owned();
            if is_unmounted_subvolume(&meta, parent_dev, mount_fstype(mounts, &path).is_some()) {
                found.push(path);
            }
        }
    }
    Ok(found)
}

fn is_unmounted_subvolume(meta: &std::fs::Metadata, parent_dev: u64, mounted: bool) -> bool {
    meta.is_dir() && meta.ino() == BTRFS_SUBVOLUME_INO && meta.dev() != parent_dev && !mounted
}

/// The views of every target, in a temporary directory that is only removed once all of them
/// are unmounted. Removing it with a view still mounted would delete the installed system.
struct Views {
    list: Vec<View>,
    dir: Option<tempfile::TempDir>,
    unmount_failed: Rc<Cell<bool>>,
}

impl Views {
    fn new(targets: &[String]) -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let base = dir.path().to_owned();
        // Owns the directory before any view is mounted, so a failure below still cleans up safely.
        let mut views = Self {
            list: vec![],
            dir: Some(dir),
            unmount_failed: Rc::default(),
        };
        for (i, target) in targets.iter().enumerate() {
            let root = base.join(i.to_string());
            let view = View::new(target, &root, Rc::clone(&views.unmount_failed))?;
            views.list.push(view);
        }
        Ok(views)
    }
}

impl Drop for Views {
    fn drop(&mut self) {
        self.list.clear();
        if self.unmount_failed.get()
            && let Some(dir) = self.dir.take()
        {
            let path = dir.keep();
            tracing::warn!(
                ?path,
                "Keeping relabel views directory, a view is still mounted"
            );
        }
    }
}

/// A private, non-recursive bind mount of one target at `<root>/<target>`, so setfiles can
/// relabel it with `-r <root>` and still match paths as they are on the installed system.
struct View {
    target: String,
    root: PathBuf,
    path: PathBuf,
    /// Set when unmounting fails, so the views directory is kept.
    unmount_failed: Rc<Cell<bool>>,
}

impl View {
    fn new(target: &str, root: &Path, unmount_failed: Rc<Cell<bool>>) -> Result<Self> {
        let path = root.join(target.trim_start_matches('/'));
        std::fs::create_dir_all(&path)?;
        nix::mount::mount(
            Some(target),
            &path,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .wrap_err_with(|| format!("cannot bind-mount {target} to relabel it"))?;
        let view = Self {
            target: target.to_owned(),
            root: root.to_owned(),
            path,
            unmount_failed,
        };
        // Keep mounts made later, including the other views, from propagating into this one.
        nix::mount::mount(
            None::<&str>,
            &view.path,
            None::<&str>,
            MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .wrap_err_with(|| format!("cannot make the {target} view private"))?;
        Ok(view)
    }

    fn setfiles_args(&self, dry_run: bool) -> Vec<String> {
        setfiles_args(&self.root, &self.path, dry_run)
    }
}

impl Drop for View {
    fn drop(&mut self) {
        if let Err(err) = nix::mount::umount2(&self.path, MntFlags::MNT_DETACH) {
            tracing::warn!(?err, path = ?self.path, "Cannot unmount relabel view");
            self.unmount_failed.set(true);
        }
    }
}

fn setfiles_args(root: &Path, path: &Path, dry_run: bool) -> Vec<String> {
    let mut args = vec!["-F".to_owned()];
    if dry_run {
        args.extend(["-n".to_owned(), "-v".to_owned()]);
    }
    args.extend([
        "-r".to_owned(),
        root.to_string_lossy().into_owned(),
        FILE_CONTEXTS.to_owned(),
        path.to_string_lossy().into_owned(),
    ]);
    args
}

/// A copy of setfiles that the installer's policy runs as `install_t`, or setfiles itself when
/// that doesn't work: the installer's policy has no `install_t`, or doesn't allow entering it.
fn install_t_setfiles() -> Result<PathBuf> {
    let dir = Path::new(SETFILES_COPY_DIR);
    std::fs::create_dir_all(dir)?;
    let copy = dir.join("setfiles");
    std::fs::copy(SETFILES, &copy).wrap_err("cannot copy setfiles")?;
    // The chroot has no selinuxfs, so libselinux can't be used here; set the label directly.
    if let Err(err) = xattr::set(&copy, "security.selinux", INSTALL_EXEC_CONTEXT) {
        tracing::warn!(
            ?err,
            "Cannot label setfiles to run as install_t, using it as is"
        );
        return Ok(SETFILES.into());
    }
    // A denied domain transition fails the exec. Without arguments setfiles only prints usage.
    if let Err(err) = Command::new(&copy).output() {
        tracing::warn!(?err, "Cannot run setfiles as install_t, using it as is");
        return Ok(SETFILES.into());
    }
    Ok(copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS: &str = "\
/dev/vdb3 / btrfs rw,seclabel,subvol=/ 0 0
/dev/vdb2 /boot ext4 rw,seclabel 0 0
/dev/vdb1 /boot/efi vfat rw,umask=0077 0 0
/dev/vdb3 /var btrfs rw,seclabel,subvol=/var 0 0
/dev/vdb3 /home btrfs rw,seclabel,subvol=/home 0 0
proc /proc proc rw 0 0
";

    #[test]
    fn relabels_every_labeled_mount() {
        let mountpoints = ["/", "/boot", "/boot/efi", "/var", "/home", "/missing"].map(Path::new);
        assert_eq!(
            relabel_targets(mountpoints.into_iter(), MOUNTS),
            ["/", "/boot", "/var", "/home"]
        );
    }

    #[test]
    fn always_relabels_root() {
        assert_eq!(relabel_targets(std::iter::empty(), MOUNTS), ["/"]);
    }

    #[test]
    fn plain_directories_are_not_subvolumes() {
        let dir = tempfile::tempdir().unwrap();
        let meta = dir.path().metadata().unwrap();
        assert!(!is_unmounted_subvolume(&meta, meta.dev(), false));
        assert!(
            unmounted_subvolumes(&["/".to_owned()], "")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn builds_setfiles_args() {
        let (root, path) = (Path::new("/tmp/v/1"), Path::new("/tmp/v/1/var"));
        assert_eq!(
            setfiles_args(root, path, false).join(" "),
            format!("-F -r /tmp/v/1 {FILE_CONTEXTS} /tmp/v/1/var")
        );
        assert_eq!(
            setfiles_args(root, path, true).join(" "),
            format!("-F -n -v -r /tmp/v/1 {FILE_CONTEXTS} /tmp/v/1/var")
        );
    }
}
