use nix::sys::time::TimeSpec;
use std::{
    os::unix::fs::{FileExt, FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
    time::SystemTime,
};

use color_eyre::eyre::{Context as _, bail, eyre};

/// Ignore errors about nonexisting files.
///
/// # Errors
///
/// Returns `r`'s error unless it is [`std::io::ErrorKind::NotFound`].
pub fn exist_then<T: Default>(r: std::io::Result<T>) -> std::io::Result<T> {
    match r {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        Err(_) => Ok(T::default()),
        Ok(x) => Ok(x),
    }
}

/// Ignore errors about nonexisting files.
///
/// # Errors
///
/// Returns an error if the directory exists but cannot be read.
pub fn exist_then_read_dir<A: AsRef<Path>>(
    p: A,
) -> std::io::Result<Box<dyn Iterator<Item = std::fs::DirEntry>>> {
    match std::fs::read_dir(p) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Box::new(std::iter::empty())),
        Err(e) => Err(e),
        Ok(x) => Ok(Box::new(x.flatten())),
    }
}
/// Removes a file if it exists, using a single syscall.
fn remove_if_exists(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path).or_else(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(e)
        }
    })
}

/// Copy directory tree from one location to another
///
/// This function wraps around different backend methods to copy a directory tree.
///
/// Currently there are two methods available:
///
/// - cp: Uses the `cp -a` command to copy the directory tree
/// - recurse: Native Rust implementation that uses `std::fs` and `jwalk` to copy the directory tree.
/// - uutils: Uses uutil's implementation of `cp` to copy the directory tree, programmatically
///
/// # Errors
///
/// Returns an error if `READYMADE_COPY_METHOD` names an unknown method, or if the copy fails.
pub fn copy_dir<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    let env = std::env::var("READYMADE_COPY_METHOD").unwrap_or_else(|_| "rdm".to_owned());
    match env.as_str() {
        "cp" => copy_dir_cp(from, to),
        #[cfg(feature = "uutils")]
        "uutils" => copy_dir_uutils(from, to),
        "recurse" | "rdm" => copy_dir_rdm(from, to),
        _ => Err(eyre!("Invalid COPY_METHOD")),
    }
}

/// Copy directory tree from one location to another using the `cp` command provided by coreutils.
///
/// This function uses the `cp -a` command to copy the directory tree.
///
/// # Errors
///
/// Returns an error if the destination cannot be created, or `cp` fails to run or exits unsuccessfully.
pub fn copy_dir_cp<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    let to = to.as_ref();
    let from = from.as_ref();
    std::fs::create_dir_all(to)?;

    tracing::info!(?from, ?to, "Copying directory using cp");

    // use cp -a to copy and preserve all attributes
    let mut process = std::process::Command::new("cp")
        .arg("-a")
        // we use /. to copy the contents of the directory, not the directory itself so it won't get nested
        .arg(format!("{from}/.", from = from.display()))
        .arg(format!("{to}/.", to = to.display()))
        .spawn()
        .map_err(|e| eyre!("Failed to spawn cp: {e}"))?;

    let status = process
        .wait()
        .map_err(|e| eyre!("Failed to wait for cp: {e}"))?;

    if !status.success() {
        bail!("Failed to copy directory");
    }

    Ok(())
}

/// Readymade's internal implementation of a FS copy
///
/// This implementation uses Rust's `std::fs` and `jwalk` to copy the directory tree.
///
/// # Errors
///
/// Returns an error if the source tree cannot be walked, or any file, directory, symlink or
/// attribute cannot be copied.
pub fn copy_dir_rdm<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    use rayon::iter::ParallelIterator;
    use rayon::prelude::*;

    /// Re-implementation of `std::fs::copy` that handles I/O efficiently, and handles symlinks properly.
    /// Also handles sparse files.
    fn copy<P: AsRef<Path>, Q: AsRef<Path>>(
        from: P,
        to: Q,
        metadata: &std::fs::Metadata,
    ) -> color_eyre::Result<()> {
        let from = from.as_ref();
        let to = to.as_ref();
        let parent = to.parent().unwrap();
        std::fs::create_dir_all(parent)?;

        remove_if_exists(to)?;

        if metadata.is_symlink() {
            let link = std::fs::read_link(from)?;
            std::os::unix::fs::symlink(link, to)?;
            return Ok(());
        }

        // Check if file is sparse
        if metadata.blocks() * 512 >= metadata.len() {
            // Not sparse, do regular copy
            std::fs::copy(from, to)?;
            return Ok(());
        }

        // File is sparse, need to copy with holes preserved
        let input = std::fs::File::open(from)?;
        let output = std::fs::File::create(to)?;
        let mut buffer = vec![0; 1024 * 1024];
        let mut offset = 0;

        loop {
            match input.read_at(&mut buffer, offset)? {
                0 => break, // EOF
                n => {
                    #[allow(clippy::indexing_slicing)]
                    if !buffer[..n].iter().all(|&x| x == 0) {
                        output.write_at(&buffer[..n], offset)?;
                    }
                    offset += n as u64;
                }
            }
        }

        Ok(())
    }

    let to = to.as_ref();
    let from = from.as_ref();
    std::fs::create_dir_all(to)?;
    tracing::info!(
        ?from,
        ?to,
        "Copying directory using internal implementation"
    );

    // Configure jwalk to use parallel traversal and disable sorting unless required
    let walkdir = jwalk::WalkDir::new(from).parallelism(jwalk::Parallelism::RayonDefaultPool {
        busy_timeout: std::time::Duration::from_millis(100),
    });

    walkdir
        .into_iter()
        .par_bridge()
        .try_for_each(|entry| -> color_eyre::Result<()> {
            let entry = entry?;
            let src_path = entry.path();
            let dest_path = to.join(src_path.strip_prefix(from)?);
            let metadata = entry.metadata()?;

            // Pre-create all directories first
            if metadata.is_dir() {
                std::fs::create_dir_all(&dest_path)?;
                copy_attributes(&src_path, &dest_path, &metadata)?;
                return Ok(());
            }

            // Handle files and symlinks
            copy(&src_path, &dest_path, &metadata)?;
            copy_attributes(&src_path, &dest_path, &metadata)?;

            Ok(())
        })?;

    std::fs::File::open(to)?.sync_all()?;

    Ok(())
}

// Convert SystemTime to TimeSpec with proper error handling
fn system_time_to_timespec(time: SystemTime) -> TimeSpec {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0));
    TimeSpec::from(duration)
}

fn copy_attributes(
    src_path: &Path,
    dest_path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), color_eyre::eyre::Error> {
    use std::os::unix::fs::MetadataExt;

    if metadata.is_symlink() {
        copy_attributes_handle_timestamps(dest_path, metadata)?;
        copy_attributes_handle_xattr(src_path, dest_path);
        return Ok(());
    }

    std::fs::set_permissions(dest_path, metadata.permissions())?;

    copy_attributes_handle_timestamps(dest_path, metadata)?;

    copy_attributes_handle_xattr(src_path, dest_path);

    let chown = nix::unistd::chown(
        dest_path,
        Some(nix::unistd::Uid::from_raw(metadata.uid())),
        Some(nix::unistd::Gid::from_raw(metadata.gid())),
    );

    if let Err(e) = chown {
        tracing::warn!("Failed to set ownership: {e}");
    }
    Ok(())
}

fn copy_attributes_handle_xattr(src_path: &Path, dest_path: &Path) {
    let xattrs = xattr::list(src_path)
        .inspect_err(|e| {
            tracing::warn!("Failed to list xattrs on {src_path:?}: {e}");
        })
        .ok()
        .into_iter()
        .flatten();
    for attr in xattrs {
        let value = match xattr::get(src_path, &attr) {
            Ok(Some(v)) => v,
            Ok(None) => Vec::new(),
            Err(e) => {
                tracing::warn!("Failed to read xattr {attr:?} on {src_path:?}: {e}");
                continue;
            }
        };

        if let Err(e) = xattr::set(dest_path, &attr, &value) {
            tracing::warn!("Failed to set xattr {attr:?} on {dest_path:?}: {e}");
        }
    }
}

#[tracing::instrument]
fn copy_attributes_handle_timestamps(
    dest_path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), color_eyre::eyre::Error> {
    use nix::sys::stat::{UtimensatFlags, utimensat};
    let atime = metadata.accessed()?;
    let mtime = metadata.modified()?;
    let atime_ts = system_time_to_timespec(atime);
    let mtime_ts = system_time_to_timespec(mtime);
    utimensat(
        nix::fcntl::AT_FDCWD,
        dest_path,
        &atime_ts,
        &mtime_ts,
        UtimensatFlags::NoFollowSymlink,
    )
    .context("Failed to set timestamps")
}

#[cfg(feature = "uutils")]
/// Copy directory tree from one location to another using uutil's implementation of `cp`.
///
/// This function requires the `uutils` feature to be enabled, and will vendor in
/// uutils' `cp` implementation to copy the directory tree.
///
/// May not be as stable as the other implementations, but is useful for testing.
///
/// # Errors
///
/// Returns an error if uutils' `cp` fails.
pub fn copy_dir_uutils<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    let opts = uu_cp::Options {
        recursive: true,
        overwrite: uu_cp::OverwriteMode::Clobber(uu_cp::ClobberMode::Force),
        attributes: uu_cp::Attributes::ALL,
        verbose: cfg!(debug_assertions),
        progress_bar: false,
        one_file_system: false,
        attributes_only: false,
        backup: uu_cp::BackupMode::None,
        copy_contents: true,
        cli_dereference: false,
        copy_mode: uu_cp::CopyMode::Copy,
        dereference: false,
        no_target_dir: false,
        parents: false,
        sparse_mode: uu_cp::SparseMode::Auto,
        strip_trailing_slashes: true,
        reflink_mode: uu_cp::ReflinkMode::Never,
        backup_suffix: "bak".into(),
        target_dir: None,
        update: uu_cp::UpdateMode::All,
        debug: cfg!(debug_assertions),
        set_selinux_context: true,
        context: None,
    };

    let from = PathBuf::from(format!("{}/.", from.as_ref().display()));
    let to = PathBuf::from(format!("{}", to.as_ref().display()));
    tracing::info!(?from, ?to, "Copying directory using uutils cp");

    uu_cp::copy(&[from], &to, &opts).map_err(|e| eyre!("Failed to copy directory: {e}"))?;

    Ok(())
}

/// Get the `/sys/dev/block/MAJ:MIN` directory of a block device. Symlinks such as
/// `/dev/disk/by-uuid/…` or `/dev/mapper/…` are followed.
fn sysfs_block_path(dev: &Path) -> color_eyre::Result<PathBuf> {
    let metadata =
        std::fs::metadata(dev).wrap_err_with(|| format!("cannot stat {}", dev.display()))?;
    if !metadata.file_type().is_block_device() {
        bail!("{} is not a block device", dev.display());
    }
    let rdev = metadata.rdev();
    Ok(PathBuf::from(format!(
        "/sys/dev/block/{}:{}",
        nix::sys::stat::major(rdev),
        nix::sys::stat::minor(rdev)
    )))
}

/// Get partition number from partition path
///
/// # Arguments
///
/// * `partition_path` - The path to the partition
///
/// # Errors
///
/// - If the partition number cannot be extracted from the partition path
/// - The path is not a valid device path
/// - The path is a whole disk, not a partition
///
/// # Example
///
/// ```no_run
/// use libreadymade::backend::util::fs::partition_number;
/// use std::path::Path;
///
/// assert_eq!(partition_number(Path::new("/dev/sda1")).unwrap(), 1);
/// assert_eq!(partition_number(Path::new("/dev/nvme0n1p2")).unwrap(), 2);
/// ```
#[tracing::instrument]
pub fn partition_number(partition_path: &Path) -> color_eyre::Result<usize> {
    let partition_number_path = sysfs_block_path(partition_path)?.join("partition");

    let partition_number = std::fs::read_to_string(partition_number_path)
        .map_err(|e| eyre!("Could not read partition number: {e}"))?
        .trim()
        .parse::<usize>()
        .map_err(|e| eyre!("Could not parse partition number: {e}"))?;

    Ok(partition_number)
}

/// Get the `major:minor` device number of a block device, e.g. `8:1` for `/dev/sda1`.
///
/// # Errors
///
/// Returns an error if `dev` is not a block device.
#[tracing::instrument]
pub fn get_maj_min(dev: &Path) -> color_eyre::Result<String> {
    let sys_path = sysfs_block_path(dev)?;
    let maj_min = (sys_path.file_name())
        .ok_or_else(|| eyre!("Could not read maj:min"))?
        .to_string_lossy()
        .into_owned();
    Ok(maj_min)
}

/// Get the whole disk from a partition path. i.e. /dev/sda1 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1
///
/// # Errors
///
/// Returns an error if `partition_path` is not a partition, or its parent device cannot be
/// resolved via sysfs.
#[tracing::instrument]
pub fn get_whole_disk(partition_path: &Path) -> color_eyre::Result<String> {
    let sys_path = sysfs_block_path(partition_path)?;
    if !sys_path.join("partition").exists() {
        bail!("{} is not a partition", partition_path.display());
    }

    // /sys/dev/block/MAJ:MIN links to /sys/devices/…/<disk>/<partition>
    let path =
        std::fs::canonicalize(&sys_path).map_err(|e| eyre!("Could not get whole disk: {e}"))?;
    let disk = (path.parent().and_then(Path::file_name))
        .ok_or_else(|| eyre!("Could not get whole disk of {}", path.display()))?;

    Ok(format!("/dev/{}", disk.to_string_lossy()))
}

/// A temporary directory to mount filesystems under.
///
/// Unlike [`tempfile::TempDir`], dropping this never deletes files: only empty directories on the
/// same filesystem as the directory itself are removed. If something under it is still mounted
/// (e.g. because unmounting failed), the mounted filesystem is left untouched.
#[derive(Debug)]
pub struct MountRoot(PathBuf);

impl MountRoot {
    /// Create a new empty temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn new() -> std::io::Result<Self> {
        Ok(Self(
            tempfile::Builder::new()
                .prefix("readymade-")
                .tempdir()?
                .keep(),
        ))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for MountRoot {
    fn drop(&mut self) {
        fn remove_empty_dirs(dir: &Path, dev: u64) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                // DirEntry::metadata() doesn't follow symlinks; mountpoints report the mounted
                // filesystem's device, so they are skipped here.
                if entry.metadata().is_ok_and(|m| m.is_dir() && m.dev() == dev) {
                    remove_empty_dirs(&entry.path(), dev);
                }
            }
            if let Err(e) = std::fs::remove_dir(dir) {
                tracing::warn!(?e, ?dir, "Cannot remove temporary mount directory");
            }
        }

        match std::fs::symlink_metadata(&self.0) {
            Ok(m) => remove_empty_dirs(&self.0, m.dev()),
            Err(e) => tracing::warn!(?e, path = ?self.0, "Cannot stat temporary mount directory"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    #[test]
    fn test_mount_root_keeps_files() {
        let root = MountRoot::new().unwrap();
        let path = root.path().to_owned();
        std::fs::create_dir_all(path.join("boot/efi")).unwrap();
        std::fs::write(path.join("boot/file"), "keep me").unwrap();
        drop(root);
        assert!(path.join("boot/file").exists());
        assert!(!path.join("boot/efi").exists());
        std::fs::remove_dir_all(path).unwrap();

        let root = MountRoot::new().unwrap();
        let path = root.path().to_owned();
        std::fs::create_dir_all(path.join("a/b/c")).unwrap();
        drop(root);
        assert!(!path.exists());
    }
    fn test_copy_impl<F>(name: &str, copy_fn: F) -> color_eyre::Result<()>
    where
        F: Fn(&Path, &Path) -> color_eyre::Result<()>,
    {
        let src: &str = &format!("/tmp/test_src_{name}");
        let dest: &str = &format!("/tmp/test_dest_{name}");

        // set up test environment
        std::fs::create_dir_all(src)?;
        std::fs::write(format!("{src}/test.txt"), "test")?;
        std::fs::set_permissions(
            format!("{src}/test.txt"),
            std::fs::Permissions::from_mode(0o700),
        )?;

        // dest
        std::fs::create_dir_all(dest)?;

        tracing::info!("Testing {} copy implementation", name);
        let o = copy_fn(Path::new(src), Path::new(dest));

        o.unwrap();
        std::fs::metadata(format!("{dest}/test.txt")).unwrap();

        // check 700 permissions
        let metadata = std::fs::metadata(format!("{dest}/test.txt"))?;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);

        // cleanup
        std::fs::remove_dir_all(src)?;
        std::fs::remove_dir_all(dest)?;

        Ok(())
    }

    #[test]
    fn test_copy_recurse() -> color_eyre::Result<()> {
        test_copy_impl("recurse", |from, to| copy_dir_rdm(from, to))
    }

    #[test]
    #[cfg(target_family = "unix")]
    fn test_copy_cp() -> color_eyre::Result<()> {
        test_copy_impl("cp", |from, to| copy_dir_cp(from, to))
    }

    #[test]
    #[cfg(feature = "uutils")]
    fn test_copy_uucp() -> color_eyre::Result<()> {
        test_copy_impl("uutils", |from, to| copy_dir_uutils(from, to))
    }
}

/// A directory to copy the new system from, mounted for as long as this lives if it had to be.
///
/// Copy sources can be a directory, or an image file or block device (such as the live ISO's
/// `/dev/mapper/live-base`) that has to be mounted first. That needs root, so it happens here in
/// the playbook rather than in the frontend.
#[derive(Debug)]
pub struct CopySource {
    path: PathBuf,
    mount: Option<MountRoot>,
}

impl CopySource {
    /// Use `source` as-is if it is a directory, otherwise mount it read-only.
    ///
    /// # Errors
    ///
    /// Returns an error if `source` doesn't exist, isn't a directory, file or block device, or
    /// cannot be mounted.
    pub fn open(source: &Path) -> color_eyre::Result<Self> {
        let meta = std::fs::metadata(source)
            .wrap_err_with(|| format!("cannot find copy source {}", source.display()))?;
        if meta.is_dir() {
            return Ok(Self {
                path: source.to_path_buf(),
                mount: None,
            });
        }
        if !meta.is_file() && !meta.file_type().is_block_device() {
            bail!(
                "copy source {} is not a directory, image or block device",
                source.display()
            );
        }

        let root = MountRoot::new()?;
        tracing::info!(?source, mountpoint = ?root.path(), "Mounting copy source");
        let output = std::process::Command::new("mount")
            .args(["-o", "ro"])
            .arg(source)
            .arg(root.path())
            .output()
            .wrap_err("cannot run mount")?;
        if !output.status.success() {
            bail!(
                "cannot mount copy source {}: {}",
                source.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(Self {
            path: root.path().to_path_buf(),
            mount: Some(root),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CopySource {
    fn drop(&mut self) {
        // the MountRoot is dropped after this, and only cleans up if this succeeded
        if self.mount.is_some() {
            match std::process::Command::new("umount")
                .arg(&self.path)
                .status()
            {
                Ok(status) if status.success() => {}
                res => tracing::warn!(?res, path = ?self.path, "Cannot unmount copy source"),
            }
        }
    }
}
