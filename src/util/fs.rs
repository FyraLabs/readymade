use std::{
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use color_eyre::eyre::{bail, eyre};

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
/// Attempt to remove a file, but ignore if the file didn't exist in the first place.
fn remove_if_exists(path: &Path) -> color_eyre::Result<()> {
    let rm = std::fs::remove_file(path);

    if rm.is_err() && rm.as_ref().unwrap_err().kind() != std::io::ErrorKind::NotFound {
        bail!(rm.unwrap_err());
    }
    Ok(())
}

/// Copy directory tree from one location to another
/// 
/// This function wraps around different backend methods to copy a directory tree.
/// 
/// Currently there are two methods available:
/// 
/// - cp: Uses the `cp -a` command to copy the directory tree
/// - recurse: Native Rust implementation that uses `std::fs` and `jwalk` to copy the directory tree, this one may be lossy and cause issues with special files
pub fn copy_dir <P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    let env = std::env::var("READYMADE_COPY_METHOD").unwrap_or_else(|_| "recurse".to_string());
    match env.as_str() {
        "cp" => copy_dir_cp(from, to),
        // "rsync" => copy_dir_rsync(from, to),
        "recurse" => copy_dir_recurse(from, to),
        _ => Err(eyre!("Invalid COPY_METHOD")),
    }
}


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
    
    let status = process.wait().map_err(|e| eyre!("Failed to wait for cp: {e}"))?;
    
    
    if !status.success() {
        bail!("Failed to copy directory");
    }

    Ok(())
}

pub fn copy_dir_recurse<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    use rayon::iter::{ParallelBridge, ParallelIterator};
    let to = to.as_ref();
    let from = from.as_ref();
    std::fs::create_dir_all(to)?;
    tracing::info!(?from, ?to, "Copying directory using Rust implementation");

    let walkdir = jwalk::WalkDir::new(from).sort(true).into_iter();

    let res = (walkdir.par_bridge()).try_for_each(|entry| -> color_eyre::Result<()> {
        let src_path = entry?.path();
        let dest_path = to.join(src_path.strip_prefix(from)?);
        let metadata = src_path.symlink_metadata()?;
        tracing::trace!(?src_path, ?dest_path, "Copying file");

        if metadata.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
        } else if metadata.is_symlink() {
            std::fs::create_dir_all(dest_path.parent().unwrap())?;
            let link = std::fs::read_link(&src_path)?;
            remove_if_exists(&dest_path)?;
            std::os::unix::fs::symlink(&link, &dest_path)?;
        } else {
            std::fs::create_dir_all(dest_path.parent().unwrap())?;
            remove_if_exists(&dest_path)?;
            std::fs::copy(&src_path, &dest_path)?;
        }


        // Apply attributes to the node,
        // but not symlinks since they'll be for the target itself
        if !metadata.is_symlink() {
            copy_attributes(&src_path, &dest_path, &metadata)?;
        }
        
        tracing::trace!(?src_path, ?dest_path, "File copy complete for file");

        Ok(())
    });
    
    
    if let Ok(()) = res {
        // sync the directory to disk
        std::fs::File::open(to)?.sync_all()?;
        Ok(())
    } else {
        Err(res.unwrap_err())
    }
}

fn to_timeval(time: std::time::SystemTime) -> nix::sys::time::TimeVal {
    let t = time.duration_since(std::time::UNIX_EPOCH).unwrap();
    nix::sys::time::TimeVal::new(
        t.as_secs().try_into().unwrap(),
        (t.as_micros() % 1_000_000).try_into().unwrap(),
    )
}

fn copy_attributes(
    src_path: &Path,
    dest_path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), color_eyre::eyre::Error> {
    let atime = metadata.accessed().expect("cannot get atime");
    let mtime = metadata.modified().expect("cannot get mtime");
    nix::sys::stat::utimes(dest_path, &to_timeval(atime), &to_timeval(mtime))?;
    let xattrs =
        xattr::list(src_path).inspect_err(|e| tracing::warn!("Failed to list xattrs: {e}"));
    (xattrs.into_iter().flat_map(IntoIterator::into_iter)).for_each(|xattr| {
        let val = xattr::get(src_path, &xattr)
            .inspect_err(|e| tracing::warn!("Failed to get xattr {xattr:?}: {e}"));
        if let Some(e) =
            (val.ok().flatten()).and_then(|val| xattr::set(dest_path, &xattr, &val).err())
        {
            tracing::warn!("Failed to set xattr {xattr:?}: {e}");
        }
    });
    
    let uid = metadata.uid();
    let gid = metadata.gid();
    nix::unistd::chown(dest_path, Some(nix::unistd::Uid::from_raw(uid)), Some(nix::unistd::Gid::from_raw(gid)))?;
    Ok(())
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
