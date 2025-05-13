use nix::sys::time::TimeSpec;
use std::{
    os::unix::fs::{FileExt, FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
    time::SystemTime,
};

use color_eyre::eyre::{bail, eyre, Context as _};

/// Ignore errors about nonexisting files.
pub fn exist_then<T: Default>(r: std::io::Result<T>) -> std::io::Result<T> {
    match r {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        Err(_) => Ok(T::default()),
        Ok(x) => Ok(x),
    }
}

/// Ignore errors about nonexisting files.
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
            let metadata = entry.metadata().expect("Cached metadata");

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
    use nix::sys::stat::{utimensat, UtimensatFlags};
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
pub fn copy_dir_uutils<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    let opts = uu_cp::Options {
        recursive: true,
        overwrite: uu_cp::OverwriteMode::Clobber(uu_cp::ClobberMode::Force),
        attributes: uu_cp::Attributes::ALL,
        verbose: cfg!(debug_assertions),
        progress_bar: false,
        one_file_system: false,
        attributes_only: false,
        backup: uu_cp::BackupMode::NoBackup,
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
        update: uu_cp::UpdateMode::ReplaceAll,
        debug: cfg!(debug_assertions),
    };

    let from = PathBuf::from(format!("{}/.", from.as_ref().display()));
    let to = PathBuf::from(format!("{}", to.as_ref().display()));
    tracing::info!(?from, ?to, "Copying directory using uutils cp");

    uu_cp::copy(&[from], &to, &opts).map_err(|e| eyre!("Failed to copy directory: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
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

/// Get partition number from partition path
///
/// # Arguments
///
/// * `partition_path` - A string slice that holds the path to the partition
///
/// # Returns
///
/// * `Result<usize>` - A string that holds the partition number
///
/// # Errors
///
/// - If the partition number cannot be extracted from the partition path
/// - The path is not a valid device path
/// - The path is a whole disk, not a partition
///
/// # Example
///
/// ```rust
///
/// let partition_path = "/dev/sda1";
/// let partition_number = get_partition_number(partition_path);
///
/// assert_eq!(partition_number.unwrap(), 1);
///
/// let partition_path = "/dev/nvme0n1p2";
/// let partition_number = get_partition_number(partition_path);
///
/// assert_eq!(partition_number.unwrap(), 2);
///
/// ```
#[tracing::instrument]
pub fn partition_number(partition_path: &str) -> color_eyre::Result<usize> {
    // first, let's
    let metadata = std::fs::metadata(partition_path)?;
    if !metadata.file_type().is_block_device() {
        bail!("Not a valid block device");
    }

    let sys_path = format!(
        "/sys/class/block/{}",
        partition_path.trim_start_matches("/dev/")
    );
    let partition_number_path = format!("{sys_path}/partition");

    let partition_number = std::fs::read_to_string(partition_number_path)
        .map_err(|e| eyre!("Could not read partition number: {e}"))?
        .trim()
        .parse::<usize>()
        .map_err(|e| eyre!("Could not parse partition number: {e}"))?;

    Ok(partition_number)
}

#[tracing::instrument]
pub fn get_maj_min(dev: &str) -> color_eyre::Result<String> {
    let sys_path = format!("/sys/class/block/{}", dev.trim_start_matches("/dev/"));

    let maj_min = std::fs::read_to_string(format!("{sys_path}/dev"))
        .map_err(|e| eyre!("Could not read maj:min: {e}"))?
        .trim()
        .to_owned();

    Ok(maj_min)
}

/// Get the whole disk from a partition path. i.e. /dev/sda1 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1
#[tracing::instrument]
pub fn get_whole_disk(partition_path: &str) -> color_eyre::Result<String> {
    let majmin = get_maj_min(partition_path)?;
    let sys_path = format!("/sys/dev/block/{majmin}");

    let path = std::fs::canonicalize(PathBuf::from(sys_path).join(".."))
        .map_err(|e| eyre!("Could not get whole disk: {e}"))?
        .to_string_lossy()
        .to_string();

    // we finally got the node name, so let's do some tiny string manipulation
    // so we can replace the /sys/devices with /dev
    let path = path
        .rsplit_once('/')
        .map_or(path.clone(), |(_, tail)| format!("/dev/{tail}"));

    Ok(path)
}
