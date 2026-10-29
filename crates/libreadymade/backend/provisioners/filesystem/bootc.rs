use crate::{
    backend::{
        mounts::generate_cryptdata, provisioners::filesystem::FileSystemProvisionerModule,
        util::fs::MountRoot,
    },
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
    /// # Errors
    ///
    /// Returns an error if `bootc install to-filesystem` fails to run or exits unsuccessfully.
    #[allow(clippy::needless_pass_by_value)]
    pub fn bootc_copy(&self, target_root: &Path, cryptdata: Option<CryptData>) -> Result<()> {
        tracing::info!(imgref=?self.imgref, "running bootc install to-filesystem");

        crate::cmd!("bootc" [
            ["install", "to-filesystem", "--source-imgref", &self.imgref],
            (cryptdata.iter())
                .flat_map(|data| data.cmdline_opts.iter().flat_map(|opt| ["--karg", opt])),
            (self.target_imgref.iter()).flat_map(|a| ["--target-imgref", a]),
            // graphical boot splash, matching what the GRUB postinstall step sets for copy installs
            ["--karg=rhgb", "--karg=quiet", "--karg=splash"],
            self.kargs.iter().flat_map(|e| ["--karg", e]),
            self.enforce_sigpolicy.then_some("--enforce-container-sigpolicy"),
            self.args.iter(),
            [target_root],
        ] => |cmd| bail!("`bootc install to-filesystem` failed: {:?}", cmd.code()));

        Ok(())
    }

    /// Clean up any file that is not on the bootc whitelist from a bootc-installed filesystem.
    ///
    /// This is best-effort: failures are logged and skipped.
    fn bootc_cleanup(mountpoint: &Path) -> Result<()> {
        for f in std::fs::read_dir(mountpoint)? {
            let f = f?;
            if let b"boot" | b"ostree" | b"efi" | b".bootc-aleph.json" =
                f.file_name().as_encoded_bytes()
            {
                continue;
            }
            let path = f.path();
            let res = if f.file_type()?.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            if let Err(e) = res {
                tracing::warn!(?e, ?path, "Cannot remove file from bootc root");
            }
        }
        Ok(())
    }

    /// Mount `mounts` under a temporary directory, run `f` on it, then unmount everything again.
    fn with_mounted<T>(
        playbook: &crate::playbook::Playbook,
        mounts: &Mounts,
        f: impl FnOnce(&Path) -> Result<T>,
    ) -> Result<T> {
        let mut mounts = mounts.clone();
        mounts.sort_mounts();

        let root = MountRoot::new()?;
        let passphrase = (playbook.encryption.as_ref()).map(|e| e.encryption_key.as_str());
        mounts.mount_all(root.path(), passphrase)?;

        let res = f(root.path());
        nix::unistd::sync();
        let umount_res = mounts.umount_all(root.path());
        let ret = res?;
        umount_res.wrap_err("cannot unmount bootc root")?;
        Ok(ret)
    }
}

impl FileSystemProvisionerModule for Bootc {
    fn run(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()> {
        let cryptdata = generate_cryptdata(mounts)?;
        Self::with_mounted(playbook, mounts, |root| self.bootc_copy(root, cryptdata))
    }

    fn cleanup(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()> {
        Self::with_mounted(playbook, mounts, Self::bootc_cleanup)
    }
}
