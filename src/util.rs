//! QoL Utilities for Readymade
use color_eyre::Section as _;

pub const LIVE_BASE: &str = "/dev/mapper/live-base";

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

// let's search for an xbootldr label
// because we never know what the device will be
pub const GRUB_CONFIG: &str = r#"search --no-floppy --label --set=dev xbootldr
set prefix=($dev)/grub2

export $prefix
configfile $prefix/grub.cfg
"#;

#[macro_export]
macro_rules! stage {
    // todo: Export text to global progress text
    ($s:literal $body:block) => {
        let s = tracing::info_span!($s);
        {
            let _guard = s.enter();
            $body
        }
    };
}
