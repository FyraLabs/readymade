//! QoL Utilities for Readymade

use std::path::Path;
pub const LIVE_BASE: &str = "/dev/mapper/live-base";
pub const ROOTFS_BASE: &str = "/run/rootfsbase";

#[cfg(target_os = "linux")]
/// Check if the current running system is UEFI or not.
///
/// Simply checks for the existence of the `/sys/firmware/efi` directory.
///
/// False negatives are possible if the system is booted in BIOS mode and the UEFI variables are not exposed.
pub fn check_uefi() -> bool {
    std::fs::metadata("/sys/firmware/efi").is_ok()
}

// macro to wrap around cmd_lib::run_fun! to prepend pkexec if not root

#[cfg(target_os = "linux")]
/// Run a command with elevated privileges if not already root.
///
/// This function relies upon the `pkexec` command to elevate privileges.
///
/// If the current user is not root, the command will be run with `pkexec`.
pub fn run_as_root(cmd: &str) -> Result<String, std::io::Error> {
    if !cmd_lib::run_fun!("whoami").unwrap().contains("root") {
        cmd_lib::run_fun!(pkexec $cmd)
    } else {
        cmd_lib::run_fun!($cmd)
    }
}

// Also, fail compilation on non-Linux platforms
#[cfg(not(target_os = "linux"))]
compile_error!(
    "Readymade does not support non-Linux platforms, these functions are Linux-specific."
);

/// Check if the current running system is a Chromebook device.
///
/// This function simply checks the existence of support for the ChromeOS embedded controller.
///
/// If the current system exposes a ChromeOS EC device, it is assumed to be a Chromebook.
///
/// There should never be a false positive since the EC device is exclusively used by Chromebooks,
/// But false negatives are possible if the device is not exposed to the current system
/// (e.g running in a VM or a container, or using a really old kernel without the I2C EC driver).
#[cfg(target_os = "linux")]
pub fn is_chromebook() -> bool {
    std::fs::metadata("/dev/cros_ec").is_ok()
}

/// Make an enum and impl Serialize
///
/// # Examples
/// ```rs
/// ini_enum! {
///     pub enum Idk {
///         A,
///         B,
///         C,
///     }
/// }
/// ```
#[macro_export]
macro_rules! ini_enum {
    (@match $field:ident) => {{
        stringify!(paste::paste! { [<$field:snake>] }).replace('_', "-")
    }};
    (@match $field:ident => $s:literal) => {{
        $s.to_string()
    }};
    (
        $(#[$outmeta:meta])*
        $v:vis enum $name:ident {
            $(
                $(#[$meta:meta])?
                $field:ident $(=> $s:literal)?,
            )*$(,)?
        }
    ) => {
        $(#[$outmeta])*
        $v enum $name {$(
            $(#[$meta])?
            $field,
        )*}
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                serializer.serialize_str(&match self {$(
                    Self::$field => ini_enum! { @match $field $(=> $s)? },
                )*})
            }
        }
    };
    (
        $(#[$outmeta1:meta])*
        $v1:vis enum $name1:ident {
            $(
                $(#[$meta1:meta])?
                $field1:ident $(=> $s1:literal)?,
            )*$(,)?
        }
        $(
            $(#[$outmeta:meta])*
            $v:vis enum $name:ident {
                $(
                    $(#[$meta:meta])?
                    $field:ident $(=> $s:literal)?,
                )*$(,)?
            }
        )+
    ) => {
        ini_enum! {
            $(
                $(#[$outmeta])*
                $v enum $name {
                    $(
                        $(#[$meta])?
                        $field $(=> $s)?,
                    )*
                }
            )+
        }
        ini_enum! {
            $(#[$outmeta1])*
            $v1 enum $name1 {
                $(
                    $(#[$meta1])?
                    $field1 $(=> $s1)?,
                )*
            }
        }
    }
}

// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// pub enum InstallStage {
//     Repart,
//     Initramfs,
//     etc...
// }

/// IPC installation message for non-interactive mode
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum InstallMessage {
    Status(String),
}

impl InstallMessage {
    pub fn new(s: &str) -> Self {
        Self::Status(s.to_owned())
    }

    pub fn into_json(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

#[macro_export]
macro_rules! stage {
    // todo: Export text to global progress text
    ($s:literal $body:block) => {{
        let s = tracing::info_span!($s);

        if std::env::var("NON_INTERACTIVE_INSTALL").is_ok_and(|v| v == "1") {
            // Then we are in a non-interactive install, which means we export IPC
            // to stdout
            let install_status = $crate::util::InstallMessage::new($s);
            println!("{}", install_status.into_json());
        }

        {
            let _guard = s.enter();
            $body
        }
    }};
}

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

pub fn copy_dir<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<(), std::io::Error> {
    use rayon::iter::{ParallelBridge, ParallelIterator};

    let to = to.as_ref();
    std::fs::create_dir_all(to)?;
    from.as_ref()
        .read_dir()?
        .par_bridge()
        .try_for_each(|dir_entry| -> std::io::Result<()> {
            let dir_entry = dir_entry?;
            let to = to.join(dir_entry.file_name());
            if dir_entry.file_type()?.is_dir() {
                copy_dir(dir_entry.path(), to)?;
            } else {
                std::fs::copy(dir_entry.path(), to)?;
            }
            Ok(())
        })
}