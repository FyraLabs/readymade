//? https://github.com/FyraLabs/readymade/blob/5acdda4ed2bc32b094b615ec3207e9a2ed458bd4/src/mksys.rs
//
// FIXME: this doesn't work due to a backhand bug that doesn't support inode id 10.
#![allow(clippy::arithmetic_side_effects)]

use backhand::{FilesystemReader, InnerNode};
use color_eyre::{Result, Section};
use std::path::Path;
use tracing::{trace, trace_span};

// TODO: somehow track progress of unsquash
/// Copy contents of a squashimg into a directory `destroot`.
/// Normally param `squashfs` should be [crate::util::DEFAULT_SQUASH_LOCATION].
#[tracing::instrument(skip(callback))]
pub fn unsquash_copy<F: FnMut(usize, usize)>(
    squashfs: &Path,
    destroot: &Path,
    mut callback: F,
) -> Result<()> {
    tracing::info!("Expanding squashfs");
    let squashimg = std::io::BufReader::new(std::fs::File::open(squashfs)?);
    let fs = FilesystemReader::from_reader(squashimg)?;
    let num_files = fs.files().count(); // WARN: might be expensive?
    let mut threads = vec![];
    let arcfs = std::sync::Arc::new(fs);
    // HACK: leak arcfs, it's a small mem size (just the arc), so it should be fine
    // prevent rust from complaining about lifetime
    let arcfs: &'static _ = Box::leak(Box::new(arcfs));
    for (i, node) in arcfs.files().enumerate() {
        callback(i - threads.len(), num_files);
        // Strip `/` else join() will output the arg (to root) directly
        let path = destroot.join(node.fullpath.strip_prefix("/")?);
        let span = trace_span!("Processing file in squashfs image", ?path);
        let _guard = span.enter();
        match &node.inner {
            InnerNode::File(f) => {
                // Just write it in split second if <= 1 MiB
                if f.file_len() <= 1024 * 1024 {
                    writef(&path, arcfs, f)?;
                    continue;
                }
                // Don't block, instead write while decompress remaining squashfs
                // Does create a bit of a problem if there are too many files in the squashfs
                // => too many threads and overwhelm the system?
                trace!("Creating thread for file creation");
                let th = std::thread::Builder::new().name(path.display().to_string());
                threads.push(th.spawn(move || writef(&path, arcfs, f))?);
            }
            InnerNode::Symlink(link) => {
                trace!(link = ?link.link, "Creating symlink");
                std::os::unix::fs::symlink(&path, &link.link)?;
            }
            x => trace!("Ignored {x:?}"),
        }
    }
    join_and_handle_threads(threads, callback, num_files)
}

fn join_and_handle_threads(
    threads: Vec<std::thread::JoinHandle<Result<(), std::io::Error>>>,
    mut callback: impl FnMut(usize, usize),
    num_files: usize,
) -> Result<()> {
    // TODO: use fold_while()?
    let mut errs = vec![];
    let l = threads.len();
    for (i, th) in threads.into_iter().enumerate() {
        callback(num_files - l + i, num_files);
        let name = th.thread().name().unwrap_or_default().to_owned();
        match th.join() {
            Ok(Err(e)) => errs.push(ParallelCopyError(name, e)),
            Err(_) => {
                // Err(_) where _ is Box<dyn Any + Send, Global>
                // This is some sort of issue with .join(), pretty useless err type, can do nothing
                let report = color_eyre::Report::msg("Fail to join thread.")
                    .note("This is a bug. Please report: https://github.com/FyraLabs/readymade")
                    .note(format!("File: {name}"));
                return Err(if errs.is_empty() {
                    report
                } else {
                    errs.into_iter().fold(
                        report.with_warning(|| "Encountered other errors below."),
                        color_eyre::Help::error,
                    )
                });
            }
            Ok(Ok(())) => {}
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs.into_iter().fold(
            color_eyre::Report::msg("Fail to extract some files."),
            color_eyre::Help::error,
        ))
    }
}

/// Internal function for writing file from unsquashfs file `f` to `path`
fn writef(
    path: &Path,
    fs: &std::sync::Arc<FilesystemReader<'_>>,
    f: &backhand::SquashfsFileReader,
) -> std::io::Result<()> {
    trace!(size = f.file_len(), "Writing file");
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::Other, "Cannot parse path parent")
    })?;
    std::fs::create_dir_all(dir)?;
    let mut reader = fs.file(f).reader();
    let mut file = std::fs::File::create_new(path)?;
    std::io::copy(&mut reader, &mut file).map(|_| ())
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    #[ignore = "We actually mount the image now, so this is not needed"]
    fn test_unsquash() -> Result<()> {
        use std::path::PathBuf;
        std::process::Command::new("mksquashfs")
            .arg("./src")
            .arg("test.sqsh")
            .status()
            .unwrap();
        unsquash_copy(
            &PathBuf::from("./test.sqsh"),
            &PathBuf::from("./test-unsquash/"),
            |_, _| (),
        )?;
        assert!(PathBuf::from("test-unsquash/mksys.rs").is_file());
        std::fs::remove_dir_all("./test-unsquash/")?;
        std::fs::remove_file("./test.sqsh")?;
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
#[error("{0}: {1:?}")]
struct ParallelCopyError(String, std::io::Error);
