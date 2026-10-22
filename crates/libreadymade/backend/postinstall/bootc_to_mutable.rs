//! Turns a root filesystem copied out of a bootc image into a traditional, package-managed
//! install that boots with GRUB and a regular initramfs, without ostree or bootupd.
//!
//! Run this after [`super::fstab::Fstab`] and before [`super::grub2::GRUB2`].

use std::os::unix::fs::PermissionsExt;

use crate::{
    backend::util::fs::{exist_then, exist_then_read_dir},
    prelude::*,
    stage,
};

use super::{Context, PostInstallModule};

/// Directories that bootc images replace with symlinks into `/var` (or drop), and the mode
/// of the real directory they're restored to.
const IMAGE_SYMLINKED_DIRS: [(&str, u32); 6] = [
    ("home", 0o755),
    ("root", 0o700),
    ("opt", 0o755),
    ("srv", 0o755),
    ("mnt", 0o755),
    ("usr/local", 0o755),
];

const USR_LOCAL_SUBDIRS: [&str; 10] = [
    "bin", "etc", "games", "include", "lib", "lib64", "libexec", "sbin", "share", "src",
];

const DRACUT_CONF: &str = "etc/dracut.conf.d/90-readymade-mutable.conf";
const DRACUT_CONF_CONTENTS: &str = "\
# Written by Readymade: this system does not boot through ostree.
omit_dracutmodules+=\" ostree bootc \"
";

/// Overrides `layout=ostree` in install.conf and earlier drop-ins (needs systemd 256+) with
/// the default, which picks `other` for GRUB's plugin on a fresh `/boot`.
const KERNEL_INSTALL_CONF: &str = "etc/kernel/install.conf.d/99-readymade.conf";
const KERNEL_INSTALL_CONF_CONTENTS: &str = "\
# Written by Readymade: this system does not boot through ostree.
layout=auto
";

/// Things in the image that only make sense on an ostree/bootc deployment.
const OSTREE_ONLY_FILES: [&str; 2] = [
    // Prepares the image for rechunking during container builds
    "etc/dnf/libdnf5-plugins/actions.d/rechunk-prep.actions",
    // The ostree symlink into /sysroot
    "ostree",
];

/// kargs.d file holding [`BootcToMutable::kargs`], so [`super::grub2::GRUB2`] adds them along
/// with the image's own.
const READYMADE_KARGS: &str = "usr/lib/bootc/kargs.d/99-readymade.toml";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct BootcToMutable {
    /// Extra kernel arguments, e.g. `install.bootc_kargs` (bootc passes them to `bootc install`).
    #[serde(default)]
    pub kargs: Vec<String>,
}

impl PostInstallModule for BootcToMutable {
    fn name(&self) -> &'static str {
        "BootcToMutable"
    }

    fn run(&self, context: &Context) -> Result<()> {
        let root = Path::new("/");

        stage!(mutable "Converting the image to a mutable system" {
            restore_filesystem_layout(root)?;
            override_ostree_config(root)?;
            disable_bootupd()?;
            write_kargs(root, &self.kargs)?;
        });

        if context.uefi {
            stage!(esp "Installing EFI bootloader files" {
                install_efi_payload(root)?;
            });
        }

        let kver = newest_kernel(root)?;
        tracing::info!(?kver, "Installing kernel");

        // kernel-install names the boot entry after the machine ID and skips GRUB's BLS plugin
        // without one. PrepareFedora clears it again so first boot gets a fresh ID.
        ensure_machine_id()?;

        stage!(kernel "Installing kernel" {
            crate::cmd!("kernel-install" [
                ["add", kver.as_str()],
                [format!("/usr/lib/modules/{kver}/vmlinuz")],
                ["--verbose"],
            ] => |r| bail!("kernel-install failed with exit code {:?}", r.code()));
            // e.g. 99-readymade.conf ignored, layout=ostree still active
            color_eyre::eyre::ensure!(
                Path::new(&format!("/boot/vmlinuz-{kver}")).exists(),
                "kernel-install did not install {kver} to /boot (kernel-install drop-ins need systemd 256+)"
            );
        });

        // kernel-install's dracut plugin usually builds this already, but run dracut explicitly so
        // the result doesn't depend on which kernel-install plugins the image ships.
        stage!(initramfs "Regenerating initramfs" {
            crate::cmd!("dracut" [
                ["--force", "--kver", kver.as_str()],
                [format!("/boot/initramfs-{kver}.img")],
            ] => |r| bail!("dracut failed with exit code {:?}", r.code()));
        });

        Ok(())
    }
}

/// Replace the `/var` symlinks bootc images use with real directories, and undo image-mode
/// defaults for new users.
fn restore_filesystem_layout(root: &Path) -> Result<()> {
    for (dir, mode) in IMAGE_SYMLINKED_DIRS {
        let path = root.join(dir);
        let is_symlink = path.symlink_metadata().is_ok_and(|m| m.is_symlink());
        if is_symlink {
            // Resolve within `root`, not the host, e.g. `/root -> var/roothome`.
            let target = std::fs::read_link(&path)?;
            let target = target.strip_prefix("/").map_or_else(
                |_| path.parent().expect("has parent").join(&target),
                |abs| root.join(abs),
            );
            std::fs::remove_file(&path)?;
            // Move the image's content (e.g. `/var/roothome/.ssh`) into place.
            if target.symlink_metadata().is_ok_and(|m| m.is_dir()) {
                tracing::debug!(?target, ?path, "Moving symlinked directory into place");
                crate::cmd!("mv" [["-T"], [&target, &path]]
                    => |r| bail!("mv {target:?} failed with exit code {:?}", r.code()));
            }
        }
        if is_symlink || !path.exists() {
            tracing::debug!(?path, "Restoring directory");
            // no-op if the image's content was moved into place
            std::fs::create_dir_all(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))?;
        }
    }
    for dir in USR_LOCAL_SUBDIRS {
        std::fs::create_dir_all(root.join("usr/local").join(dir))?;
    }

    for file in OSTREE_ONLY_FILES {
        exist_then(std::fs::remove_file(root.join(file)))?;
    }

    let useradd = root.join("etc/default/useradd");
    if let Ok(contents) = std::fs::read_to_string(&useradd) {
        let fixed: String = (contents.split_inclusive('\n'))
            .map(|line| match line.trim_end_matches('\n') {
                "HOME=/var/home" => line.replacen("/var/home", "/home", 1),
                _ => line.to_owned(),
            })
            .collect();
        if fixed != contents {
            std::fs::write(&useradd, fixed)?;
        }
    }
    Ok(())
}

/// Mask configuration that tells dnf, kernel-install and dracut they're on an ostree system.
fn override_ostree_config(root: &Path) -> Result<()> {
    // dnf: the image keeps a single kernel and lets it be replaced while running, which is
    // only safe when updates are staged into a new deployment. Masking restores the defaults.
    let dnf_user_dir = root.join("etc/dnf/libdnf5.conf.d");
    for entry in exist_then_read_dir(root.join("usr/share/dnf5/libdnf.conf.d"))? {
        let name = entry.file_name();
        if name.to_string_lossy().contains("ostree") {
            tracing::debug!(?name, "Masking ostree dnf config");
            std::fs::create_dir_all(&dnf_user_dir)?;
            std::fs::write(
                dnf_user_dir.join(&name),
                "# Masked by Readymade: this system is not ostree-based.\n",
            )?;
        }
    }

    // dracut: the image adds the ostree and bootc modules, which expect an ostree deployment.
    // kernel-install: layout=ostree hands kernels to rpm-ostree instead of installing them.
    for (path, contents) in [
        (DRACUT_CONF, DRACUT_CONF_CONTENTS),
        (KERNEL_INSTALL_CONF, KERNEL_INSTALL_CONF_CONTENTS),
    ] {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("has parent"))?;
        std::fs::write(path, contents)?;
    }
    Ok(())
}

/// bootupd manages the ESP on image-based systems only; GRUB is installed directly here.
/// Image-mode auto-update units are masked too: they'd fail or fight with dnf.
fn disable_bootupd() -> Result<()> {
    const UNITS: [&str; 5] = [
        "bootloader-update.service",
        "bootc-fetch-apply-updates.timer",
        "bootc-fetch-apply-updates.service",
        "rpm-ostreed-automatic.timer",
        "rpm-ostreed-automatic.service",
    ];
    let units = UNITS
        .into_iter()
        .filter(|unit| {
            ["/usr/lib/systemd/system", "/etc/systemd/system"]
                .iter()
                .any(|dir| Path::new(dir).join(unit).exists())
        })
        .collect_vec();
    if !units.is_empty() {
        // mask refuses to replace an existing unit in /etc: --force handles symlinks, but
        // regular files have to go first (they're being masked anyway)
        for unit in &units {
            let path = Path::new("/etc/systemd/system").join(unit);
            if path.symlink_metadata().is_ok_and(|m| m.is_file()) {
                std::fs::remove_file(&path)
                    .wrap_err_with(|| format!("cannot remove {}", path.display()))?;
            }
        }
        crate::cmd!("systemctl" [["mask", "--force"], &units]
            => |r| bail!("systemctl mask {units:?} failed with exit code {:?}", r.code()));
    }
    Ok(())
}

/// Copy shim and GRUB onto the ESP from the packages' payload in `/usr/lib/efi`, the way
/// their RPMs lay them out under `/boot/efi` on a package-based install.
fn install_efi_payload(root: &Path) -> Result<()> {
    let esp = root.join("boot/efi/EFI");
    // /usr/lib/efi/<package>/<version>/EFI, newest version of each package only
    let mut payloads = exist_then_read_dir(root.join("usr/lib/efi"))?
        .filter_map(|pkg| {
            exist_then_read_dir(pkg.path())
                .ok()?
                .map(|ver| ver.path().join("EFI"))
                .filter(|dir| dir.is_dir())
                .max_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()))
        })
        .collect_vec();
    payloads.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));

    std::fs::create_dir_all(&esp)?;
    for payload in &payloads {
        tracing::info!(?payload, ?esp, "Copying EFI payload");
        // FAT can't store ownership, permissions, xattrs or symlinks
        crate::cmd!("cp" [["-rLT", "--no-preserve=all"], [payload, &esp]]
            => |r| bail!("cp {payload:?} failed with exit code {:?}", r.code()));
    }

    // Need the fallback loader in EFI/BOOT and shim or GRUB in a vendor directory. FAT is
    // case-insensitive, so match names the same way.
    let lowercase_name =
        |e: &std::fs::DirEntry| e.file_name().to_string_lossy().to_ascii_lowercase();
    let (mut fallback, mut loader) = (false, false);
    for dir in exist_then_read_dir(&esp)?.filter(|e| e.path().is_dir()) {
        let is_boot = lowercase_name(&dir) == "boot";
        let names = exist_then_read_dir(dir.path()).into_iter().flatten();
        let is_efi = |name: &String| Path::new(name).extension().is_some_and(|ext| ext == "efi");
        for name in names.map(|e| lowercase_name(&e)).filter(is_efi) {
            fallback |= is_boot && name.starts_with("boot");
            loader |= !is_boot && (name.starts_with("shim") || name.starts_with("grub"));
        }
    }
    if !fallback || !loader {
        bail!("No shim/GRUB EFI binaries found in /usr/lib/efi or on the ESP");
    }
    Ok(())
}

fn ensure_machine_id() -> Result<()> {
    let machine_id = std::fs::read_to_string("/etc/machine-id").unwrap_or_default();
    if matches!(machine_id.trim(), "" | "uninitialized") {
        crate::cmd!("systemd-machine-id-setup"
            => |r| bail!("systemd-machine-id-setup failed with exit code {:?}", r.code()));
    }
    Ok(())
}

/// Kernels in `/usr/lib/modules` that ship a `vmlinuz`.
pub(super) fn installed_kernels(root: &Path) -> Result<Vec<String>> {
    let modules = root.join("usr/lib/modules");
    Ok(std::fs::read_dir(&modules)
        .wrap_err_with(|| format!("cannot read {}", modules.display()))?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().join("vmlinuz").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect())
}

/// Newest kernel with a `vmlinuz`, preferring non-`+debug` ones.
fn newest_kernel(root: &Path) -> Result<String> {
    installed_kernels(root)?
        .into_iter()
        // Prefer regular kernels over `+debug` variants of the same or older version.
        .max_by(|a, b| {
            (!a.ends_with("+debug"))
                .cmp(&!b.ends_with("+debug"))
                .then_with(|| natural_cmp(a, b))
        })
        .ok_or_else(|| eyre!("No kernel found in {:?}", root.join("usr/lib/modules")))
}

/// Compare strings with runs of digits compared numerically, so `6.10` sorts after `6.9`.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, &str)> {
        s.as_bytes()
            .chunk_by(|x, y| x.is_ascii_digit() == y.is_ascii_digit())
            .map(|c| {
                let c = std::str::from_utf8(c).unwrap_or_default();
                (c.starts_with(|ch: char| ch.is_ascii_digit()), c)
            })
            .collect()
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for (&(da, xa), &(db, xb)) in ca.iter().zip(&cb) {
        let ord = if da && db {
            let (ta, tb) = (xa.trim_start_matches('0'), xb.trim_start_matches('0'));
            ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb))
        } else {
            xa.cmp(xb)
        };
        if ord.is_ne() {
            return ord;
        }
    }
    ca.len().cmp(&cb.len())
}

/// Save `kargs` in kargs.d for [`image_kargs`] to pick up.
fn write_kargs(root: &Path, kargs: &[String]) -> Result<()> {
    #[derive(Serialize)]
    struct KargsFile<'a> {
        kargs: &'a [String],
    }

    if kargs.is_empty() {
        return Ok(());
    }
    let path = root.join(READYMADE_KARGS);
    std::fs::create_dir_all(path.parent().expect("has parent"))?;
    std::fs::write(path, toml::to_string(&KargsFile { kargs })?)?;
    Ok(())
}

/// Kernel arguments a bootc image declares in `/usr/lib/bootc/kargs.d`. bootc applies these
/// itself; mutable installs need them added to the GRUB command line.
///
/// See <https://bootc-dev.github.io/bootc/building/kernel-arguments.html>.
///
/// # Errors
/// - a kargs file can't be read or parsed
pub fn image_kargs(root: &Path) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct KargsFile {
        kargs: Vec<String>,
        match_architectures: Option<Vec<String>>,
    }

    let mut files = exist_then_read_dir(root.join("usr/lib/bootc/kargs.d"))?
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect_vec();
    files.sort();

    let mut kargs = vec![];
    for path in files {
        let file: KargsFile = toml::from_str(&std::fs::read_to_string(&path)?)
            .wrap_err_with(|| format!("cannot parse {}", path.display()))?;
        if (file.match_architectures.as_ref())
            .is_none_or(|archs| archs.iter().any(|a| a == std::env::consts::ARCH))
        {
            kargs.extend(file.kargs);
        }
    }
    Ok(kargs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{MetadataExt, symlink},
    };

    #[test]
    fn restores_symlinked_dirs() -> Result<()> {
        let root = tempfile::tempdir()?;
        let root = root.path();
        fs::create_dir_all(root.join("var/roothome/.ssh"))?;
        fs::create_dir_all(root.join("var/opt/app"))?;
        fs::create_dir_all(root.join("srv/keep"))?; // a real directory
        fs::create_dir_all(root.join("usr"))?;
        fs::create_dir_all(root.join("var/usrlocal/bin"))?;
        fs::write(root.join("var/usrlocal/bin/tool"), "")?;
        fs::create_dir_all(root.join("etc/default"))?;
        symlink("var/roothome", root.join("root"))?;
        symlink("/var/opt", root.join("opt"))?;
        symlink("../var/usrlocal", root.join("usr/local"))?;
        symlink("var/home", root.join("home"))?; // dangling
        symlink("sysroot/ostree", root.join("ostree"))?;
        let useradd = "#HOME=/var/home\nHOME=/var/home\nA=1\nHOME=/var/home";
        fs::write(root.join("etc/default/useradd"), useradd)?;

        restore_filesystem_layout(root)?;

        for dir in ["root/.ssh", "opt/app", "srv/keep", "home", "usr/local/bin"] {
            assert!(root.join(dir).symlink_metadata()?.is_dir(), "{dir}");
        }
        assert_eq!(root.join("root").metadata()?.mode() & 0o777, 0o700);
        assert!(!root.join("var/roothome").exists());
        assert!(root.join("usr/local/bin/tool").is_file());
        assert!(!root.join("var/usrlocal").exists());
        root.join("ostree").symlink_metadata().unwrap_err();
        assert_eq!(
            fs::read_to_string(root.join("etc/default/useradd"))?,
            "#HOME=/var/home\nHOME=/home\nA=1\nHOME=/home"
        );
        Ok(())
    }

    #[test]
    fn masks_ostree_config() -> Result<()> {
        let root = tempfile::tempdir()?;
        let root = root.path();
        let dnf = root.join("usr/share/dnf5/libdnf.conf.d");
        fs::create_dir_all(&dnf)?;
        fs::write(dnf.join("20-ostree-installonlypkgs.conf"), "")?;
        fs::write(dnf.join("30-other.conf"), "")?;

        override_ostree_config(root)?;

        let masked = root.join("etc/dnf/libdnf5.conf.d");
        assert!(masked.join("20-ostree-installonlypkgs.conf").exists());
        assert!(!masked.join("30-other.conf").exists());
        for (path, contents) in [
            (DRACUT_CONF, DRACUT_CONF_CONTENTS),
            (KERNEL_INSTALL_CONF, KERNEL_INSTALL_CONF_CONTENTS),
        ] {
            assert_eq!(fs::read_to_string(root.join(path))?, contents);
        }
        Ok(())
    }

    #[test]
    fn copies_efi_payload() -> Result<()> {
        let root = tempfile::tempdir()?;
        let root = root.path();
        let shim = root.join("usr/lib/efi/shim/16.1-7/EFI");
        fs::create_dir_all(shim.join("BOOT"))?;
        fs::create_dir_all(shim.join("fedora"))?;
        fs::write(shim.join("BOOT/BOOTX64.EFI"), "shim")?;
        fs::write(shim.join("fedora/shimx64.efi"), "shim")?;
        let grub = root.join("usr/lib/efi/grub2/1:2.12-66.fc44/EFI/fedora");
        fs::create_dir_all(&grub)?;
        fs::write(grub.join("grubx64.efi"), "grub")?;
        let old_grub = root.join("usr/lib/efi/grub2/1:2.12-7.fc44/EFI/fedora");
        fs::create_dir_all(&old_grub)?;
        fs::write(old_grub.join("grubx64.efi"), "old")?;

        install_efi_payload(root)?;

        let esp = root.join("boot/efi/EFI");
        assert!(esp.join("BOOT/BOOTX64.EFI").is_file());
        assert!(esp.join("fedora/shimx64.efi").is_file());
        assert_eq!(fs::read_to_string(esp.join("fedora/grubx64.efi"))?, "grub");
        Ok(())
    }

    #[test]
    fn errors_without_efi_payload() -> Result<()> {
        let root = tempfile::tempdir()?;
        assert!(install_efi_payload(root.path()).is_err());
        // e.g. Windows' fallback loader only
        let boot = root.path().join("boot/efi/EFI/Boot");
        fs::create_dir_all(&boot)?;
        fs::write(boot.join("bootx64.efi"), "")?;
        assert!(install_efi_payload(root.path()).is_err());
        Ok(())
    }

    #[test]
    fn picks_newest_kernel() -> Result<()> {
        let root = tempfile::tempdir()?;
        let modules = root.path().join("usr/lib/modules");
        for kver in [
            "6.9.12-200.fc44.x86_64",
            "6.10.3-200.fc44.x86_64",
            "6.11.0-1.fc44.x86_64",
            "6.10.3-200.fc44.x86_64+debug",
        ] {
            fs::create_dir_all(modules.join(kver))?;
            if kver != "6.11.0-1.fc44.x86_64" {
                fs::write(modules.join(kver).join("vmlinuz"), "")?;
            }
        }
        // 6.11 has no vmlinuz (e.g. leftover modules), so it isn't a candidate
        assert_eq!(newest_kernel(root.path())?, "6.10.3-200.fc44.x86_64");
        Ok(())
    }

    #[test]
    fn natural_ordering() {
        use std::cmp::Ordering::*;
        assert_eq!(natural_cmp("6.10.0", "6.9.0"), Greater);
        assert_eq!(natural_cmp("6.9.0", "6.9.0"), Equal);
        assert_eq!(natural_cmp("1:2.12-66", "1:2.12-7"), Greater);
        assert_eq!(natural_cmp("6.9", "6.9.1"), Less);
    }

    #[test]
    fn reads_image_kargs() -> Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("usr/lib/bootc/kargs.d");
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join("00-nvidia.toml"),
            r#"kargs = ["rd.driver.blacklist=nouveau", "nvidia-drm.modeset=1"]"#,
        )?;
        fs::write(
            dir.join("10-other-arch.toml"),
            r#"kargs = ["foo"]
match-architectures = ["not-an-arch"]"#,
        )?;
        fs::write(dir.join("README"), "ignored")?;

        assert_eq!(
            image_kargs(root.path())?,
            vec!["rd.driver.blacklist=nouveau", "nvidia-drm.modeset=1"]
        );
        assert_eq!(
            image_kargs(&root.path().join("missing"))?,
            Vec::<String>::new()
        );

        write_kargs(root.path(), &["console=ttyS0".into(), "a=\"b c\"".into()])?;
        assert_eq!(
            image_kargs(root.path())?,
            vec![
                "rd.driver.blacklist=nouveau",
                "nvidia-drm.modeset=1",
                "console=ttyS0",
                "a=\"b c\""
            ]
        );
        Ok(())
    }
}
