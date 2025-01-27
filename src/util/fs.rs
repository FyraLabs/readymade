use std::{
    os::unix::fs::FileTypeExt,
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

pub fn copy_dir_cp<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    let to = to.as_ref();
    let from = from.as_ref();
    std::fs::create_dir_all(to)?;

    
    // use cp -a to copy and preserve all attributes
    let mut process = std::process::Command::new("cp")
        .arg("-a")
        .arg(from)
        .arg(to)
        .spawn()
        .map_err(|e| eyre!("Failed to spawn cp: {e}"))?;
    
    let status = process.wait().map_err(|e| eyre!("Failed to wait for cp: {e}"))?;
    
    
    if !status.success() {
        bail!("Failed to copy directory");
    }

    Ok(())
}

pub fn copy_dir<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    use rayon::iter::{ParallelBridge, ParallelIterator};

    let to = to.as_ref();
    let from = from.as_ref();
    std::fs::create_dir_all(to)?;

    let walkdir = jwalk::WalkDir::new(from).sort(true).into_iter();

    (walkdir.par_bridge()).try_for_each(|entry| -> color_eyre::Result<()> {
        let src_path = entry?.path();
        let dest_path = to.join(src_path.strip_prefix(from)?);
        let metadata = src_path.symlink_metadata()?;

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

        // set attributes only for files and dirs, symlinks will fail with ENOENT
        if metadata.is_dir() || metadata.is_file() {
            set_attributes(&src_path, &dest_path, &metadata)?;
        }

        Ok(())
    })
}

fn to_timeval(time: std::time::SystemTime) -> nix::sys::time::TimeVal {
    let t = time.duration_since(std::time::UNIX_EPOCH).unwrap();
    nix::sys::time::TimeVal::new(
        t.as_secs().try_into().unwrap(),
        (t.as_micros() % 1_000_000).try_into().unwrap(),
    )
}
fn set_attributes(
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
