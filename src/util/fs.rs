use std::{
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

use color_eyre::{
    eyre::{bail, eyre},
    Section,
};

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

pub fn copy_dir<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> color_eyre::Result<()> {
    use rayon::iter::{ParallelBridge, ParallelIterator};

    let to = to.as_ref();
    std::fs::create_dir_all(to)?;
    from.as_ref()
        .read_dir()?
        .par_bridge()
        .try_for_each(|dir_entry| -> color_eyre::Result<()> {
            let dir_entry = dir_entry?;
            let to = to.join(dir_entry.file_name());
            let metadata = dir_entry.path().symlink_metadata().map_err(|e| {
                eyre!("can't grab metadata")
                    .note(format!("Path : {}", dir_entry.path().display()))
                    .wrap_err(e)
            })?;
            if metadata.is_dir() {
                copy_dir(dir_entry.path(), to)?;
            } else if metadata.is_symlink() {
                let link = std::fs::read_link(dir_entry.path())?;
                std::os::unix::fs::symlink(&link, &to).map_err(|e| {
                    eyre!("can't symlink")
                        .note(format!("From : {}", dir_entry.path().display()))
                        .note(format!("To   : {}", to.display()))
                        .note(format!("Link : {}", link.display()))
                        .wrap_err(e)
                })?;
            } else {
                std::fs::copy(dir_entry.path(), &to).map_err(|e| {
                    eyre!("can't copy file")
                        .note(format!("From : {}", dir_entry.path().display()))
                        .note(format!("To   : {}", to.display()))
                        .wrap_err(e)
                })?;
            }
            Ok(())
        })
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
        .map_or(path.clone(), |(_, tail)| tail.to_owned());

    Ok(path)
}

// we can't reliably test the disk functions since
// it accesses the system's block devices + sysfs

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test_partition_number() {

//         let partition_path = "/dev/nvme0n1p2";
//         let partno = partition_number(partition_path);

//         assert_eq!(partno.unwrap(), 2);
//     }

//     #[test]
//     fn test_get_whole_disk() {
//         // let partition_path = "/dev/sda1";
//         // let whole_disk = get_whole_disk(partition_path);

//         // assert_eq!(whole_disk, "/dev/sda");

//         let partition_path = "/dev/nvme0n1p2";
//         let whole_disk = get_whole_disk(partition_path);
//         println!("{:?}", whole_disk);

//         // assert_eq!(whole_disk, "/dev/nvme0n1");
//     }
// }
