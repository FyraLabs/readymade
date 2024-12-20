use std::path::Path;

use color_eyre::{eyre::eyre, Section};

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
