use crate::{
    backend::{mounts::generate_cryptdata, provisioners::filesystem::FileSystemProvisionerModule},
    prelude::*,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Bootc {
    pub imgref: String,
    pub target_imgref: Option<String>,
    pub enforce_sigpolicy: bool,
    pub kargs: Vec<String>,
    pub args: Vec<String>,
}

impl Bootc {
    /// Call bootc to copy the contents of the container into the target.
    ///
    /// The caller must verify that `self.copy_mode.is_bootc()`.
    #[allow(clippy::unwrap_in_result, clippy::needless_pass_by_value)]
    pub fn bootc_copy(&self, target_root: &Path, cryptdata: Option<CryptData>) -> Result<()> {
        let imgref = &self.imgref;
        let target_imgref = &self.target_imgref;
        let enforce_sigpolicy = &self.enforce_sigpolicy;
        let args = &self.args;

        tracing::info!(imgref=?self.imgref, "running bootc install to-filesystem");

        crate::cmd!("bootc" [
            ["install", "to-filesystem", "--source-imgref", imgref],
            (cryptdata.iter())
                .flat_map(|data| data.cmdline_opts.iter().flat_map(|opt| ["--karg", opt])),
            ["--karg=rhgb", "--karg=quiet", "--karg=splash"],
            [target_root],
            (target_imgref.iter()).flat_map(|a| ["--target-imgref", a]),
            args.iter().flat_map(|e| ["--karg", e]),
            enforce_sigpolicy.then_some("--enforce-container-sigpolicy"),
            args.iter(),
        ] => |cmd| bail!("`bootc install to-filesystem` failed: {:?}", cmd.code()));

        Ok(())
    }

    // This cleans up any folder that is not on the bootc whitelist from a bootc-installed filesystem
    fn bootc_cleanup(mountpoint: &Path) -> Result<()> {
        _ = std::fs::read_dir(mountpoint)?.try_for_each(|f| {
            let f = f?;
            match f.file_name().as_encoded_bytes() {
                b"boot" | b"ostree" | b"efi" | b".bootc-aleph.json" => {}
                _ => {
                    _ = if f.file_type()?.is_dir() {
                        std::fs::remove_dir_all(f.path())
                    } else {
                        std::fs::remove_file(f.path())
                    }
                }
            }
            std::io::Result::Ok(())
        });
        Ok(())
    }
}

impl FileSystemProvisionerModule for Bootc {
    fn run(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()> {
        let tmproot = tempfile::tempdir()?;
        let bootc_rootfs_mountpoint = tmproot.path();
        mounts.mount_all(
            bootc_rootfs_mountpoint,
            playbook
                .encryption
                .as_ref()
                .map(|e| e.encryption_key.as_str()),
        );

        self.bootc_copy(bootc_rootfs_mountpoint, generate_cryptdata(mounts)?)?;

        mounts.umount_all(bootc_rootfs_mountpoint);
        Ok(())
    }

    fn cleanup(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()> {
        let tmproot = tempfile::tempdir()?;
        let bootc_rootfs_mountpoint = tmproot.path();
        mounts.mount_all(
            bootc_rootfs_mountpoint,
            playbook
                .encryption
                .as_ref()
                .map(|e| e.encryption_key.as_str()),
        );
        Self::bootc_cleanup(bootc_rootfs_mountpoint)?;
        crate::cmd!("sync" => |_| bail!("`sync` failed"));
        crate::cmd!("umount" [["-R"], [bootc_rootfs_mountpoint]] => |_| bail!("umount -R {bootc_rootfs_mountpoint:?} failed"));
        Ok(())
    }
}
