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

/// Where GRUB2 writes `grub.cfg`. grub2-common owns it on package-based installs, but bootc
/// images move `/boot` to `/usr/lib/ostree-boot` and ship an empty one.
const GRUB_DIR: &str = "boot/grub2";

/// Where genhomedircon, which regenerates the `SELinux` home directory contexts
/// (`file_contexts.homedirs`), may live: `/usr/sbin` is only merged into `/usr/bin` on some images.
const GENHOMEDIRCON: [&str; 2] = ["usr/bin/genhomedircon", "usr/sbin/genhomedircon"];

/// Things in the image that only make sense on an ostree/bootc deployment.
const OSTREE_ONLY_FILES: [&str; 2] = [
    // Prepares the image for rechunking during container builds
    "etc/dnf/libdnf5-plugins/actions.d/rechunk-prep.actions",
    // The ostree symlink into /sysroot
    "ostree",
];

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BootcToMutable;

impl PostInstallModule for BootcToMutable {
    fn name(&self) -> &'static str {
        "BootcToMutable"
    }

    fn run(&self, context: &Context) -> Result<()> {
        let root = Path::new("/");

        stage!(mutable "Converting the image to a mutable system" {
            restore_filesystem_layout(root)?;
            create_grub_dir(root)?;
            override_ostree_config(root)?;
            disable_bootupd(root)?;
        });

        // The image's policy generated its home directory contexts while useradd's HOME was
        // /var/home, which semanage skips (/var is in its ignoredirs). That leaves no rules for
        // /home, so it and every home directory would be labeled default_t.
        if GENHOMEDIRCON.iter().any(|path| root.join(path).exists()) {
            stage!(homedirs "Updating SELinux home directory contexts" {
                crate::cmd!("genhomedircon"
                    => |r| bail!("genhomedircon failed with exit code {:?}", r.code()));
            });
        }

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
            std::fs::remove_file(&path)?;
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

/// Create [`GRUB_DIR`] with grub2-common's mode, so the GRUB2 module can write `grub.cfg`.
fn create_grub_dir(root: &Path) -> Result<()> {
    let dir = root.join(GRUB_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
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
fn disable_bootupd(root: &Path) -> Result<()> {
    const UNIT: &str = "bootloader-update.service";
    if root.join("usr/lib/systemd/system").join(UNIT).exists() {
        crate::cmd!("systemctl" [["mask", UNIT]]
            => |r| bail!("systemctl mask {UNIT} failed with exit code {:?}", r.code()));
    }
    Ok(())
}

/// Copy shim and GRUB onto the ESP from the packages' payload in `/usr/lib/efi`, the way
/// their RPMs lay them out under `/boot/efi` on a package-based install.
fn install_efi_payload(root: &Path) -> Result<()> {
    let esp = root.join("boot/efi/EFI");
    // /usr/lib/efi/<package>/<version>/EFI
    let mut payloads = crate::backend::util::fs::exist_then_read_dir(root.join("usr/lib/efi"))?
        .filter_map(|pkg| crate::backend::util::fs::exist_then_read_dir(pkg.path()).ok())
        .flatten()
        .map(|ver| ver.path().join("EFI"))
        .filter(|dir| dir.is_dir())
        .collect_vec();
    payloads.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));

    for payload in &payloads {
        tracing::info!(?payload, ?esp, "Copying EFI payload");
        copy_tree_plain(payload, &esp)?;
    }

    if !esp.join("BOOT").is_dir() && payloads.is_empty() {
        bail!("No EFI bootloader files found in /usr/lib/efi or on the ESP");
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
        .max_by(|a, b| natural_cmp(a, b))
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
        root.join("ostree").symlink_metadata().unwrap_err();
        assert_eq!(
            std::fs::read_to_string(root.join("etc/default/useradd")).unwrap(),
            "GROUP=100\nHOME=/home\nSHELL=/bin/bash\n"
        );
    }

    #[test]
    fn creates_grub_dir() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        // bootc images ship an empty /boot
        std::fs::create_dir(root.join("boot")).unwrap();

        create_grub_dir(root).unwrap();

        let meta = root.join(GRUB_DIR).metadata().unwrap();
        assert!(meta.is_dir());
        assert_eq!(meta.permissions().mode() & 0o777, 0o700);

        // an existing directory is left alone
        std::fs::write(root.join(GRUB_DIR).join("grubenv"), "").unwrap();
        create_grub_dir(root).unwrap();
        assert!(root.join(GRUB_DIR).join("grubenv").exists());
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

        override_ostree_config(root).unwrap();

        let masked = root.join("etc/dnf/libdnf5.conf.d");
        assert!(masked.join("20-ostree-installonlypkgs.conf").exists());
        assert!(!masked.join("30-other.conf").exists());
        let install_conf = std::fs::read_to_string(root.join("etc/kernel/install.conf")).unwrap();
        assert!(install_conf.starts_with("# layout=ostree"));
        assert!(!root.join("etc/kernel/install.conf.d/50-gen.conf").exists());
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

        install_efi_payload(root).unwrap();

        let esp = root.join("boot/efi/EFI");
        assert!(esp.join("BOOT/BOOTX64.EFI").is_file());
        assert!(esp.join("fedora/shimx64.efi").is_file());
        assert!(esp.join("fedora/grubx64.efi").is_file());
    }

    #[test]
    fn errors_without_efi_payload() {
        let root = tempfile::tempdir().unwrap();
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
    }
}
