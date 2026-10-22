//! Turns a root filesystem copied out of a bootc image into a traditional, package-managed
//! install that boots with GRUB and a regular initramfs, without ostree or bootupd.
//!
//! Run this after [`super::fstab::Fstab`] and before [`super::grub2::GRUB2`].

use std::os::unix::fs::PermissionsExt;

use crate::{backend::util::fs::exist_then, prelude::*, stage};

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
            disable_bootupd(root)?;
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
        ensure_machine_id(root)?;

        stage!(kernel "Installing kernel" {
            crate::cmd!("kernel-install" [
                ["add", kver.as_str()],
                [format!("/usr/lib/modules/{kver}/vmlinuz")],
                ["--verbose"],
            ] => |r| bail!("kernel-install failed with exit code {:?}", r.code()));
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
                match std::fs::rename(&target, &path) {
                    Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                        crate::cmd!("mv" [["-T"], [&target, &path]]
                            => |r| bail!("mv {target:?} failed with exit code {:?}", r.code()));
                    }
                    r => r.wrap_err_with(|| format!("cannot move {}", target.display()))?,
                }
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))?;
                continue;
            }
        }
        if is_symlink || !path.exists() {
            tracing::debug!(?path, "Restoring directory");
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
        let fixed = contents
            .lines()
            .map(|line| {
                if line == "HOME=/var/home" {
                    "HOME=/home"
                } else {
                    line
                }
            })
            .join("\n")
            + "\n";
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
    for entry in
        crate::backend::util::fs::exist_then_read_dir(root.join("usr/share/dnf5/libdnf.conf.d"))?
    {
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

    // kernel-install: layout=ostree hands kernels to rpm-ostree instead of installing them.
    let etc_install_conf = root.join("etc/kernel/install.conf");
    let install_conf = if etc_install_conf.exists() {
        etc_install_conf.clone()
    } else {
        root.join("usr/lib/kernel/install.conf")
    };
    let dropins = ["usr/lib/kernel/install.conf.d", "etc/kernel/install.conf.d"]
        .into_iter()
        .map(|dir| crate::backend::util::fs::exist_then_read_dir(root.join(dir)))
        .flatten_ok()
        .map_ok(|entry| {
            let dest = root
                .join("etc/kernel/install.conf.d")
                .join(entry.file_name());
            (entry.path(), dest)
        })
        // An /etc drop-in overrides /usr/lib one of the same name; it's handled on its own.
        .filter_ok(|(src, dest)| src == dest || !dest.exists())
        .collect::<std::io::Result<Vec<_>>>()?;
    for (src, dest) in std::iter::once((install_conf, etc_install_conf)).chain(dropins) {
        let Ok(contents) = std::fs::read_to_string(&src) else {
            continue;
        };
        if let Some(fixed) = drop_ostree_layout(&contents) {
            tracing::debug!(?src, ?dest, "Overriding kernel-install layout=ostree");
            std::fs::create_dir_all(dest.parent().expect("has parent"))?;
            std::fs::write(&dest, fixed)?;
        }
    }

    // dracut: the image adds the ostree and bootc modules, which expect an ostree deployment.
    let dracut_conf = root.join(DRACUT_CONF);
    std::fs::create_dir_all(dracut_conf.parent().expect("has parent"))?;
    std::fs::write(dracut_conf, DRACUT_CONF_CONTENTS)?;
    Ok(())
}

/// Comment out `layout=ostree`, returning `None` if there's nothing to change.
fn drop_ostree_layout(contents: &str) -> Option<String> {
    let is_ostree_layout = |line: &str| {
        line.split_once('=')
            .is_some_and(|(k, v)| k.trim() == "layout" && v.trim() == "ostree")
    };
    contents.lines().any(is_ostree_layout).then(|| {
        contents
            .lines()
            .map(|line| {
                if is_ostree_layout(line) {
                    format!("# {line} (disabled by Readymade: not an ostree system)")
                } else {
                    line.to_owned()
                }
            })
            .join("\n")
            + "\n"
    })
}

/// bootupd manages the ESP on image-based systems only; GRUB is installed directly here.
/// Image-mode auto-update units are masked too: they'd fail or fight with dnf.
fn disable_bootupd(root: &Path) -> Result<()> {
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
            ["usr/lib/systemd/system", "etc/systemd/system"]
                .iter()
                .any(|dir| root.join(dir).join(unit).exists())
        })
        .collect_vec();
    if !units.is_empty() {
        // mask refuses to replace an existing unit in /etc: --force handles symlinks, but
        // regular files have to go first (they're being masked anyway)
        for unit in &units {
            let path = root.join("etc/systemd/system").join(unit);
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
    let mut payloads = crate::backend::util::fs::exist_then_read_dir(root.join("usr/lib/efi"))?
        .filter_map(|pkg| {
            crate::backend::util::fs::exist_then_read_dir(pkg.path())
                .ok()?
                .map(|ver| ver.path().join("EFI"))
                .filter(|dir| dir.is_dir())
                .max_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()))
        })
        .collect_vec();
    payloads.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));

    for payload in &payloads {
        tracing::info!(?payload, ?esp, "Copying EFI payload");
        copy_tree_plain(payload, &esp)?;
    }

    // FAT is case-insensitive, so match names the same way.
    let has = |dir: &Path, pred: &dyn Fn(&str) -> bool| {
        crate::backend::util::fs::exist_then_read_dir(dir).is_ok_and(|mut entries| {
            entries.any(|e| pred(&e.file_name().to_string_lossy().to_ascii_lowercase()))
        })
    };
    let subdirs = crate::backend::util::fs::exist_then_read_dir(&esp)?
        .filter(|e| e.path().is_dir())
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_ascii_lowercase(),
                e.path(),
            )
        })
        .collect_vec();
    let is_efi = |f: &str| {
        Path::new(f)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("efi"))
    };
    let fallback = subdirs
        .iter()
        .any(|(name, dir)| name == "boot" && has(dir, &|f| f.starts_with("boot") && is_efi(f)));
    let loader = subdirs.iter().any(|(name, dir)| {
        name != "boot"
            && has(dir, &|f| {
                (f.starts_with("shim") || f.starts_with("grub")) && is_efi(f)
            })
    });
    if !fallback || !loader {
        bail!("No shim/GRUB EFI binaries found in /usr/lib/efi or on the ESP");
    }
    Ok(())
}

/// Copy a directory tree without preserving ownership, permissions or xattrs, which the ESP's
/// FAT filesystem can't store.
fn copy_tree_plain(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree_plain(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest).wrap_err_with(|| {
                format!(
                    "cannot copy {} to {}",
                    entry.path().display(),
                    dest.display()
                )
            })?;
        }
    }
    Ok(())
}

fn ensure_machine_id(root: &Path) -> Result<()> {
    let machine_id = std::fs::read_to_string(root.join("etc/machine-id")).unwrap_or_default();
    if matches!(machine_id.trim(), "" | "uninitialized") {
        crate::cmd!("systemd-machine-id-setup"
            => |r| bail!("systemd-machine-id-setup failed with exit code {:?}", r.code()));
    }
    Ok(())
}

/// The newest kernel in `/usr/lib/modules` that ships a `vmlinuz`.
fn newest_kernel(root: &Path) -> Result<String> {
    let modules = root.join("usr/lib/modules");
    std::fs::read_dir(&modules)
        .wrap_err_with(|| format!("cannot read {}", modules.display()))?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().join("vmlinuz").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        // Prefer regular kernels over `+debug` variants of the same or older version.
        .max_by(|a, b| {
            (!a.ends_with("+debug"))
                .cmp(&!b.ends_with("+debug"))
                .then_with(|| natural_cmp(a, b))
        })
        .ok_or_else(|| eyre!("No kernel found in {modules:?}"))
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

    let mut files =
        crate::backend::util::fs::exist_then_read_dir(root.join("usr/lib/bootc/kargs.d"))?
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
    use std::os::unix::fs::symlink;

    #[test]
    fn restores_symlinked_dirs() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        std::fs::create_dir_all(root.join("usr")).unwrap();
        std::fs::create_dir_all(root.join("etc/default")).unwrap();
        std::fs::create_dir_all(root.join("sysroot/ostree")).unwrap();
        symlink("var/home", root.join("home")).unwrap();
        symlink("var/roothome", root.join("root")).unwrap();
        symlink("sysroot/ostree", root.join("ostree")).unwrap();
        symlink("../var/usrlocal", root.join("usr/local")).unwrap();
        symlink("/var/opt", root.join("opt")).unwrap();
        std::fs::create_dir_all(root.join("var/roothome/.ssh")).unwrap();
        std::fs::write(root.join("var/roothome/.ssh/authorized_keys"), "key").unwrap();
        std::fs::create_dir_all(root.join("var/opt/app")).unwrap();
        std::fs::create_dir_all(root.join("var/usrlocal/bin")).unwrap();
        std::fs::write(root.join("var/usrlocal/bin/tool"), "").unwrap();
        // a real directory (e.g. a mounted /srv) is left alone
        std::fs::create_dir_all(root.join("srv")).unwrap();
        std::fs::write(root.join("srv/keep"), "").unwrap();
        std::fs::write(
            root.join("etc/default/useradd"),
            "GROUP=100\nHOME=/var/home\nSHELL=/bin/bash\n",
        )
        .unwrap();

        restore_filesystem_layout(root).unwrap();

        for dir in [
            "home",
            "root",
            "opt",
            "srv",
            "mnt",
            "usr/local",
            "usr/local/bin",
        ] {
            let meta = root.join(dir).symlink_metadata().unwrap();
            assert!(meta.is_dir(), "{dir} should be a directory");
        }
        assert_eq!(
            root.join("root").metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(root.join("srv/keep").exists());
        assert!(root.join("root/.ssh/authorized_keys").is_file());
        assert!(root.join("opt/app").is_dir());
        assert!(root.join("usr/local/bin/tool").is_file());
        assert!(!root.join("var/roothome").exists());
        root.join("ostree").symlink_metadata().unwrap_err();
        assert_eq!(
            std::fs::read_to_string(root.join("etc/default/useradd")).unwrap(),
            "GROUP=100\nHOME=/home\nSHELL=/bin/bash\n"
        );
    }

    #[test]
    fn masks_ostree_config() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let dnf = root.join("usr/share/dnf5/libdnf.conf.d");
        std::fs::create_dir_all(&dnf).unwrap();
        std::fs::write(
            dnf.join("20-ostree-installonlypkgs.conf"),
            "[main]\ninstallonlypkgs=''\n",
        )
        .unwrap();
        std::fs::write(dnf.join("30-other.conf"), "[main]\n").unwrap();
        let kernel = root.join("usr/lib/kernel");
        std::fs::create_dir_all(kernel.join("install.conf.d")).unwrap();
        std::fs::write(kernel.join("install.conf"), "layout=ostree\n").unwrap();
        std::fs::write(
            kernel.join("install.conf.d/50-gen.conf"),
            "initrd_generator=dracut\n",
        )
        .unwrap();
        std::fs::write(
            kernel.join("install.conf.d/60-layout.conf"),
            "layout=ostree\n",
        )
        .unwrap();
        let etc_dropins = root.join("etc/kernel/install.conf.d");
        std::fs::create_dir_all(&etc_dropins).unwrap();
        std::fs::write(etc_dropins.join("60-layout.conf"), "layout=bls\n").unwrap();

        override_ostree_config(root).unwrap();

        let masked = root.join("etc/dnf/libdnf5.conf.d");
        assert!(masked.join("20-ostree-installonlypkgs.conf").exists());
        assert!(!masked.join("30-other.conf").exists());
        let install_conf = std::fs::read_to_string(root.join("etc/kernel/install.conf")).unwrap();
        assert!(install_conf.starts_with("# layout=ostree"));
        assert!(!root.join("etc/kernel/install.conf.d/50-gen.conf").exists());
        assert_eq!(
            std::fs::read_to_string(etc_dropins.join("60-layout.conf")).unwrap(),
            "layout=bls\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(DRACUT_CONF)).unwrap(),
            DRACUT_CONF_CONTENTS
        );
    }

    #[test]
    fn leaves_non_ostree_layout_alone() {
        assert_eq!(drop_ostree_layout("layout=bls\n"), None);
        assert_eq!(
            drop_ostree_layout("a=b\n layout = ostree\n").unwrap(),
            "a=b\n#  layout = ostree (disabled by Readymade: not an ostree system)\n"
        );
    }

    #[test]
    fn copies_efi_payload() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let shim = root.join("usr/lib/efi/shim/16.1-7/EFI");
        std::fs::create_dir_all(shim.join("BOOT")).unwrap();
        std::fs::create_dir_all(shim.join("fedora")).unwrap();
        std::fs::write(shim.join("BOOT/BOOTX64.EFI"), "shim").unwrap();
        std::fs::write(shim.join("fedora/shimx64.efi"), "shim").unwrap();
        let grub = root.join("usr/lib/efi/grub2/1:2.12-66.fc44/EFI/fedora");
        std::fs::create_dir_all(&grub).unwrap();
        std::fs::write(grub.join("grubx64.efi"), "grub").unwrap();
        let old_grub = root.join("usr/lib/efi/grub2/1:2.12-7.fc44/EFI/fedora");
        std::fs::create_dir_all(&old_grub).unwrap();
        std::fs::write(old_grub.join("grubx64.efi"), "old").unwrap();

        install_efi_payload(root).unwrap();

        let esp = root.join("boot/efi/EFI");
        assert!(esp.join("BOOT/BOOTX64.EFI").is_file());
        assert!(esp.join("fedora/shimx64.efi").is_file());
        assert_eq!(
            std::fs::read_to_string(esp.join("fedora/grubx64.efi")).unwrap(),
            "grub"
        );
    }

    #[test]
    fn errors_without_efi_payload() {
        let root = tempfile::tempdir().unwrap();
        assert!(install_efi_payload(root.path()).is_err());
        // e.g. Windows' fallback loader only
        let boot = root.path().join("boot/efi/EFI/Boot");
        std::fs::create_dir_all(&boot).unwrap();
        std::fs::write(boot.join("bootx64.efi"), "").unwrap();
        assert!(install_efi_payload(root.path()).is_err());
    }

    #[test]
    fn picks_newest_kernel() {
        let root = tempfile::tempdir().unwrap();
        let modules = root.path().join("usr/lib/modules");
        for kver in [
            "6.9.12-200.fc44.x86_64",
            "6.10.3-200.fc44.x86_64",
            "6.11.0-1.fc44.x86_64",
            "6.10.3-200.fc44.x86_64+debug",
        ] {
            std::fs::create_dir_all(modules.join(kver)).unwrap();
            if kver != "6.11.0-1.fc44.x86_64" {
                std::fs::write(modules.join(kver).join("vmlinuz"), "").unwrap();
            }
        }
        // 6.11 has no vmlinuz (e.g. leftover modules), so it isn't a candidate
        assert_eq!(
            newest_kernel(root.path()).unwrap(),
            "6.10.3-200.fc44.x86_64"
        );
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
    fn reads_image_kargs() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("usr/lib/bootc/kargs.d");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("00-nvidia.toml"),
            r#"kargs = ["rd.driver.blacklist=nouveau", "nvidia-drm.modeset=1"]"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("10-other-arch.toml"),
            r#"kargs = ["foo"]
match-architectures = ["not-an-arch"]"#,
        )
        .unwrap();
        std::fs::write(dir.join("README"), "ignored").unwrap();

        assert_eq!(
            image_kargs(root.path()).unwrap(),
            vec!["rd.driver.blacklist=nouveau", "nvidia-drm.modeset=1"]
        );
        assert_eq!(
            image_kargs(&root.path().join("missing")).unwrap(),
            Vec::<String>::new()
        );

        write_kargs(root.path(), &["console=ttyS0".into(), "a=\"b c\"".into()]).unwrap();
        assert_eq!(
            image_kargs(root.path()).unwrap(),
            vec![
                "rd.driver.blacklist=nouveau",
                "nvidia-drm.modeset=1",
                "console=ttyS0",
                "a=\"b c\""
            ]
        );
    }
}
