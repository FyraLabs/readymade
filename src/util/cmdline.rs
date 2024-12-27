//! kernel cmdline manager, used to manage kernel command line arguments
//! edits /etc/kernel/cmdline file

use color_eyre::eyre::bail;
use color_eyre::Result;
use std::fs::File;
use std::io::{Read, Write};

/// `KernelCmdline` is a helper struct to manage kernel command line arguments.
/// It serializes the kernel arguments as [`Vec<String>`] and writes them `to /etc/kernel/cmdline`
#[allow(clippy::module_name_repetitions)]
pub struct KernelCmdline {
    file_handle: File,
    internal_vec: Vec<String>,
}

impl KernelCmdline {
    /// Create a new handle to /etc/kernel/cmdline\

    pub fn new(file: &mut File) -> Result<Self> {
        let internal_vec = Self::read_file_handle(file)?;
        Ok(Self {
            file_handle: file.try_clone()?,
            internal_vec,
        })
    }

    pub fn from_root() -> Result<Self> {
        let mut file_handle = {
            if std::fs::metadata("/etc/kernel/cmdline").is_ok() {
                std::fs::OpenOptions::new()
                    .write(true)
                    .read(true)
                    .open("/etc/kernel/cmdline")?
            } else if std::fs::create_dir_all("/etc/kernel").is_err() {
                bail!("Failed to create /etc/kernel directory");
            } else {
                std::fs::File::create("/etc/kernel/cmdline")?
            }
        };

        let internal_vec = Self::read_file_handle(&mut file_handle)?;
        Ok(Self {
            file_handle,
            internal_vec,
        })

        // Ok(Self { file_handle , internal_vec: self.read()? })
    }

    /// Re-reads the /etc/kernel/cmdline file
    fn read_file_handle(file_handle: &mut File) -> Result<Vec<String>> {
        let mut cmdline = String::new();
        file_handle.read_to_string(&mut cmdline)?;
        Ok(cmdline
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect())
    }

    pub fn get(&self) -> Vec<String> {
        self.internal_vec.clone()
    }

    /// Sets all kernel command line arguments
    pub fn set(&mut self, args: &[String]) {
        self.internal_vec = args.to_vec();
    }

    /// Appends to /etc/kernel/cmdline
    ///
    /// NOTE: Make sure to call `write()` to commit the changes, as this
    /// operation is atomic
    pub fn append(&mut self, arg: String) {
        self.internal_vec.push(arg);
    }

    /// Commits all changes to /etc/kernel/cmdline
    pub fn write(&mut self) -> Result<()> {
        let cmdline = self.internal_vec.join(" ");
        self.file_handle.write_all(cmdline.as_bytes())?;
        Ok(())
    }

    /// Checks if a kernel argument exists already, and replace if it does.
    /// May be useful for replacing certain arguments
    ///
    /// NOTE: Make sure to call `write()` to commit the changes, as this operation is atomic
    pub fn append_or_replace(&mut self, arg: &str) {
        // do nothing if arg already exists
        if self.internal_vec.iter().any(|x| x.contains(arg)) {
            return;
        }

        if let Some((key, _)) = arg.split_once('=') {
            if let Some(pos) = self.internal_vec.iter().position(|x| x.starts_with(key)) {
                if let Some(element) = self.internal_vec.get_mut(pos) {
                    arg.clone_into(element);
                    return;
                }
            }
        }
        self.internal_vec.push(arg.to_owned());
    }
}

impl Drop for KernelCmdline {
    fn drop(&mut self) {
        drop(self.flush());
        drop(self.file_handle.sync_all());
    }
}

impl Read for KernelCmdline {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file_handle.read(buf)
    }
}

impl Write for KernelCmdline {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file_handle.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file_handle.flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::{Seek, Write};
    use tempfile::tempfile;

    const READ_TEST: &str = "root=UUID=1234 ro";

    #[test]
    fn test_read_cmdline() {
        let mut file = tempfile().unwrap();
        file.write_all(READ_TEST.as_bytes()).unwrap();
        file.seek(std::io::SeekFrom::Start(0)).unwrap();

        let cmdline = KernelCmdline::new(&mut file).unwrap();
        assert_eq!(cmdline.get(), vec!["root=UUID=1234", "ro"]);
    }

    #[test]
    fn test_append_cmdline() {
        let mut file = tempfile().unwrap();
        file.write_all(READ_TEST.as_bytes()).unwrap();
        file.seek(std::io::SeekFrom::Start(0)).unwrap();

        let mut cmdline = KernelCmdline::new(&mut file).unwrap();
        cmdline.append("quiet".to_owned());
        println!("{:?}", cmdline.get());
        assert_eq!(cmdline.get(), vec!["root=UUID=1234", "ro"]);
    }

    #[test]
    fn test_append_or_replace() {
        let mut file = tempfile().unwrap();
        file.write_all(READ_TEST.as_bytes()).unwrap();
        file.seek(std::io::SeekFrom::Start(0)).unwrap();

        let mut cmdline = KernelCmdline::new(&mut file).unwrap();
        cmdline.append_or_replace("root=UUID=5678");
        cmdline.append_or_replace("rhgb");
        cmdline.append_or_replace("quiet");
        assert_eq!(cmdline.get(), vec!["root=UUID=5678", "ro", "rhgb", "quiet"]);

        cmdline.append_or_replace("quiet");
        assert_eq!(cmdline.get(), vec!["root=UUID=5678", "ro", "rhgb", "quiet"]);

        cmdline.append_or_replace("root=LABEL=nyaa");
        assert_eq!(cmdline.get(), vec!["root=LABEL=nyaa", "ro", "rhgb", "quiet"]);
    }

    #[test]
    fn test_set_cmdline() {
        let mut file = tempfile().unwrap();
        file.write_all(READ_TEST.as_bytes()).unwrap();
        file.seek(std::io::SeekFrom::Start(0)).unwrap();

        let mut cmdline = KernelCmdline::new(&mut file).unwrap();
        cmdline.set(&["test".to_owned()]);
        assert_eq!(cmdline.get(), vec!["test"]);
    }
}
