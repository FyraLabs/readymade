use std::path::Path;

use color_eyre::{eyre::{bail, eyre}, Section};

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
/// * `Result<String>` - A string that holds the partition number
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
    if !partition_path.starts_with("/dev/") {
        bail!("Not a valid device path");
    }

    // Table of known block device prefixes
    // Simple number suffix: /dev/sdXN, /dev/vdXN
    // pY suffix: /dev/nvmeXpY, /dev/mmcblkXpY, /dev/loopXpY
    if partition_path.starts_with("/dev/sd") || partition_path.starts_with("/dev/vd") {
        let partition_number = partition_path
            .chars()
            .skip_while(|c| c.is_alphabetic())
            .filter(|c| c.is_numeric())
            .collect::<String>();

        return Ok(partition_number.parse::<usize>()?);
    }

    if partition_path.starts_with("/dev/nvme")
        || partition_path.starts_with("/dev/mmcblk")
        || partition_path.starts_with("/dev/loop")
    {
        let partition_number = partition_path
            .chars()
            .skip_while(|c| c.is_alphabetic())
            .skip_while(|c| c.is_numeric())
            .skip_while(|c| *c != 'p')
            .skip(1)
            .take_while(|c| c.is_numeric())
            .collect::<String>();

        if !partition_number.is_empty() {
            return Ok(partition_number.parse::<usize>()?);
        }

        bail!("Could not extract partition number");
    }

    bail!("Could not extract partition number");
}

/// Get the whole disk from a partition path. i.e. /dev/sda1 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1
#[tracing::instrument]
pub fn get_whole_disk(partition_path: &str) -> String {
    if partition_path.starts_with("/dev/sd") || partition_path.starts_with("/dev/vd") {
        if let Some(pos) = partition_path.rfind(|c: char| c.is_numeric()) {
            return partition_path[..pos].to_string();
        }
    }

    if partition_path.starts_with("/dev/nvme")
        || partition_path.starts_with("/dev/mmcblk")
        || partition_path.starts_with("/dev/loop")
    {
        // split by p
        let mut parts = partition_path.split('p');
        let partition_path = parts.next().unwrap();
        return partition_path.to_owned();
    }

    partition_path.to_owned()
}
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_partition_number() {
        let partition_path = "/dev/sda1";
        let partno = partition_number(partition_path);

        assert_eq!(partno.unwrap(), 1);

        let partition_path = "/dev/nvme0n1p2";
        let partno = partition_number(partition_path);

        assert_eq!(partno.unwrap(), 2);
    }

    #[test]
    fn test_get_whole_disk() {
        let partition_path = "/dev/sda1";
        let whole_disk = get_whole_disk(partition_path);

        assert_eq!(whole_disk, "/dev/sda");

        let partition_path = "/dev/nvme0n1p2";
        let whole_disk = get_whole_disk(partition_path);

        assert_eq!(whole_disk, "/dev/nvme0n1");
    }
}
