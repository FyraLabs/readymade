//! Keeps the NVIDIA kernel module maintainable on mutable installs of an NVIDIA image.
//!
//! NVIDIA images build the module with akmods at image build time and mask akmods' key
//! generation, since there's no persistent key in a container build. On a mutable install the
//! module has to be rebuilt by akmods on every kernel update, signed with a machine-owned key
//! that the firmware trusts, so this module:
//!
//! - checks `akmod-nvidia` is installed (it does nothing for images without it)
//! - unmasks `akmods-keygen` and generates the signing key (always a fresh one on Secure Boot
//!   systems, so a key shipped in the image is never enrolled)
//! - on Secure Boot systems, rebuilds the module with that key (the image's module was signed
//!   with a throwaway build key) and queues enrollment of the key with `mokutil`, confirmed in
//!   `MokManager` on first reboot
//!
//! Run this after the kernel and bootloader are installed, and before [`super::selinux::SELinux`]
//! so the generated keys get labeled.

use crate::{prelude::*, stage};

use super::{Context, PostInstallModule};

const AKMODS_PUBLIC_KEY: &str = "/etc/pki/akmods/certs/public_key.der";
const AKMODS_PRIVATE_KEY: &str = "/etc/pki/akmods/private/private_key.priv";
const AKMODS_KEYGEN_UNITS: [&str; 2] = [
    "akmods-keygen.target",
    "akmods-keygen@akmods-keygen.service",
];

/// Password used when none is configured.
pub const DEFAULT_MOK_PASSWORD: &str = "readymade";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct NvidiaAkmods {
    /// Password the user types in `MokManager` on first boot to confirm enrolling the akmods
    /// key. Readymade sets it from `install.nvidia_mok_password`, which the UI shows.
    pub mok_password: String,
}

impl Default for NvidiaAkmods {
    fn default() -> Self {
        Self {
            mok_password: DEFAULT_MOK_PASSWORD.into(),
        }
    }
}

impl PostInstallModule for NvidiaAkmods {
    fn name(&self) -> &'static str {
        "NvidiaAkmods"
    }

    fn run(&self, context: &Context) -> Result<()> {
        let akmod_installed = Command::new("rpm")
            .args(["-q", "--quiet", "akmod-nvidia"])
            .status()
            .wrap_err("cannot run rpm")?
            .success();
        if !akmod_installed {
            tracing::info!("akmod-nvidia is not installed, skipping NVIDIA setup");
            return Ok(());
        }

        let enroll = context.uefi && crate::backend::util::sys::secure_boot_enabled();
        stage!(nvidia "Setting up NVIDIA driver updates" {
            crate::cmd!("systemctl" [["unmask"], AKMODS_KEYGEN_UNITS]
                => |r| bail!("systemctl unmask akmods-keygen failed with exit code {:?}", r.code()));
            // Rebuilds modules for new kernels at boot if the kernel-install hook didn't
            crate::cmd!("systemctl" [["enable", "akmods.service"]]
                => |r| bail!("systemctl enable akmods.service failed with exit code {:?}", r.code()));

            // akmods-keygen would create the key on first boot, but it has to exist now to be
            // queued for enrollment. A key shipped in the image is shared by everyone who pulled
            // it, so never enroll that one: force a fresh, machine-owned key instead.
            if enroll {
                crate::cmd!("kmodgenca" [["-a", "-f"]]
                    => |r| bail!("kmodgenca failed with exit code {:?}", r.code()));
            } else if !Path::new(AKMODS_PUBLIC_KEY).exists()
                || !Path::new(AKMODS_PRIVATE_KEY).exists()
            {
                crate::cmd!("kmodgenca" [["-a"]]
                    => |r| bail!("kmodgenca failed with exit code {:?}", r.code()));
            }
        });

        if enroll {
            stage!(nvidia "Rebuilding NVIDIA driver with the new signing key" {
                rebuild_kmods()?;
            });
            stage!(mok "Queueing Secure Boot key enrollment" {
                queue_mok_enrollment(Path::new(AKMODS_PUBLIC_KEY), &self.mok_password)?;
            });
        }

        Ok(())
    }
}

/// Rebuild the kmod for every installed kernel, since akmods won't rebuild a kmod that already
/// exists. Failures only warn: the system still boots, and akmods rebuilds and signs the module
/// with the enrolled key on the next kernel update.
fn rebuild_kmods() -> Result<()> {
    for kver in super::bootc_to_mutable::installed_kernels(Path::new("/"))? {
        if !run_or_warn(
            Command::new("akmods").args(["--force", "--kernels", &kver]),
            "akmods failed, NVIDIA module won't load under Secure Boot until the next kernel update",
        ) {
            continue;
        }
        // The initramfs may carry an early-loaded copy of the module with the old signature.
        // Only regenerate existing ones: kernels without a boot entry don't get one.
        let initramfs = format!("/boot/initramfs-{kver}.img");
        if Path::new(&initramfs).exists() {
            run_or_warn(
                Command::new("dracut").args(["--force", "--kver", &kver, &initramfs]),
                "dracut failed",
            );
        }
    }
    Ok(())
}

/// Run `cmd`, logging `msg` as a warning if it fails. Returns whether it succeeded.
fn run_or_warn(cmd: &mut Command, msg: &str) -> bool {
    match cmd.status() {
        Ok(status) if status.success() => return true,
        Ok(status) => tracing::warn!(?cmd, code = ?status.code(), "{msg}"),
        Err(err) => tracing::warn!(?cmd, ?err, "{msg}"),
    }
    false
}

/// Ask shim to enroll `key` on next boot. The user confirms it in `MokManager` with `password`.
fn queue_mok_enrollment(key: &Path, password: &str) -> Result<()> {
    let hash = Command::new("mokutil")
        .arg(format!("--generate-hash={password}"))
        .output()
        .wrap_err("cannot run mokutil")?;
    if !hash.status.success() {
        bail!(
            "mokutil --generate-hash failed: {}",
            String::from_utf8_lossy(&hash.stderr)
        );
    }
    let hash_file = tempfile::NamedTempFile::new()?;
    std::fs::write(hash_file.path(), &hash.stdout)?;

    crate::cmd!("mokutil" [
        ["--import"], [key],
        ["--hash-file"], [hash_file.path()],
    ] => |r| bail!("mokutil --import failed with exit code {:?}", r.code()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::postinstall::Module;

    #[test]
    fn deserializes_without_password() {
        let module: Module = serde_json::from_str(r#"{"module":"NvidiaAkmods"}"#).unwrap();
        assert_eq!(module, NvidiaAkmods::default().into());
        let module: Module =
            serde_json::from_str(r#"{"module":"NvidiaAkmods","mok_password":"secret"}"#).unwrap();
        assert_eq!(
            module,
            NvidiaAkmods {
                mok_password: "secret".into()
            }
            .into()
        );
    }
}
