//! Install NVIDIA, Broadcom (wifi and bluetooth) drivers automatically after installation.
//!
//! See <https://github.com/Ultramarine-Linux/stellar/blob/v0.2.0/umstellar/driver.py>
use color_eyre::{
    eyre::{eyre, Context as _},
    Section,
};
use serde::{Deserialize, Serialize};

use super::{Context, PostInstallModule};

static NVIDIA_PREFIXES: std::sync::LazyLock<std::collections::HashMap<&str, &str>> =
    std::sync::LazyLock::new(|| {
        [
            // List of chipset prefixes with its corresponding last supported driver version
            // if it's not in the list, it's probably supported by the latest driver
            // but... if it's really really old, then you're out of luck
            // We're gonna be supporting GPUs from the 8000 series and up
            ("NV", "unsupported"),
            ("MCP", "unsupported"),
            ("G7", "unsupported"),
            ("G8", "340xx"),
            // wtf this goes from like 8000 to 100 series
            ("G9", "340xx"),
            // finally, a sane naming scheme
            // Tesla GPUs
            ("GT", "340xx"),
            // Fermi GPUs, in case you like burning your house down
            ("GF", "390xx"),
            // Kepler GPUs
            // now we're finally up to the modern era
            ("GK", "470xx"),
            // The rest should be supported by the latest driver, at least as of
            // late 2023
        ]
        .into_iter()
        .collect()
    });

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Drivers;

impl PostInstallModule for Drivers {
    fn run(&self, _context: &Context) -> color_eyre::Result<()> {
        Self::setup_nvidia()?;
        Self::setup_broadcom()?;
        Ok(())
    }
}

impl Drivers {
    /// Returns the latest supported driver for the given chipset
    fn get_nvidia_driver(chipset: &str) -> &str {
        for (prefix, driver) in &*NVIDIA_PREFIXES {
            if chipset.starts_with(prefix) {
                return driver;
            }
        }
        "latest"
    }

    fn check_nvidia_gpu() -> bool {
        std::process::Command::new("sh")
            .arg("-c")
            .arg("lspci | grep -q -i NVIDIA")
            .status()
            .is_ok_and(|s| s.success())
    }

    // todo: Probably just filter programatically instead of doing this pipeline
    fn get_nvidia_chipset() -> color_eyre::Result<String> {
        String::from_utf8(std::process::Command::new("sh").arg("-c").arg("lspci | grep -i NVIDIA | head -n 1 | cut -d ':' -f 3 | cut -d '[' -f 1 | sed -e 's/^[[:space:]]*//'").stdout(std::process::Stdio::piped()).output().wrap_err("cannot detect nvidia chipset")?.stdout.rsplit(|&c| c == b' ').next().expect("malformatted output from shell").to_vec()).wrap_err("cannot convert shell output to utf8")
    }

    ///    Returns a list of Nvidia packages to install
    fn list_nvidia_packages() -> color_eyre::Result<Vec<String>> {
        let mut pkgs = vec!["nvidia-gpu-firmware".into(), "libva-nvidia-driver".into()];
        let chipset = Self::get_nvidia_chipset()?;
        match Self::get_nvidia_driver(&chipset) {
            "unsupported" => (),
            "latest" => pkgs.extend_from_slice(&[
                "akmod-nvidia".into(),
                "xorg-x11-drv-nvidia".into(),
                "xorg-x11-drv-nvidia-cuda".into(),
            ]),
            v => pkgs.extend_from_slice(&[
                format!("akmod-nvidia-{v}"),
                format!("xorg-x11-drv-nvidia-{v}"),
                format!("xorg-x11-drv-nvidia-{v}-cuda"),
            ]),
        }
        Ok(pkgs)
    }

    fn is_ostree() -> bool {
        std::fs::exists("/ostree").unwrap_or_default()
    }

    fn setup_nvidia_ostree() -> color_eyre::Result<()> {
        let pkgs = Self::list_nvidia_packages()?;
        let p = std::process::Command::new("rpm-ostree")
            .args(["install", "-y"])
            .args(&pkgs)
            .status()
            .wrap_err("fail to run `rpm-ostree`")?;
        if !p.success() {
            return Err(eyre!("fail to install rpm-ostree packages")
                .note(format!("pkgs={pkgs:?}"))
                .note(format!("exit code: {:?}", p.code())));
        }

        let p = std::process::Command::new("rpm-ostree")
            .args([
                "kargs",
                "--append=rd.driver.blacklist=nouveau",
                "--append=modprobe.blacklist=nouveau",
                "--append=nvidia-drm.modeset=1",
                "initcall_blacklist=simpledrm_platform_driver_init",
            ])
            .args(pkgs)
            .status()
            .wrap_err("fail to run `rpm-ostree`")?;
        if !p.success() {
            return Err(
                eyre!("fail to run rpm-ostree kargs").note(format!("exit code: {:?}", p.code()))
            );
        }
        Ok(())
    }

    fn setup_nvidia() -> color_eyre::Result<()> {
        tracing::info!("Setting up Nvidia drivers");
        let primary_gpu = std::env::var("STELLAR_OPTION").is_ok_and(|x| x == "1");
        if !Self::check_nvidia_gpu() {
            tracing::info!("No Nvidia GPU detected");
            return Ok(());
        }
        if Self::is_ostree() {
            tracing::debug!("ostree detected");
            return Self::setup_nvidia_ostree();
        }

        let pkgs = Self::list_nvidia_packages()?;
        let p = std::process::Command::new("dnf")
            .args(["in", "-y", "--allowerasing", "--best"])
            .args(&pkgs)
            .status()
            .wrap_err("fail to run `dnf`")?;
        if !p.success() {
            return Err(eyre!("fail to install nvidia pkgs")
                .note(format!("pkgs={pkgs:?}"))
                .note(format!("exit code from dnf: {:?}", p.code())));
        }

        if primary_gpu {
            let p = std::process::Command::new("sh")
                .arg("-c")
                .arg(
                    r#"
                        cp -p /usr/share/X11/xorg.conf.d/nvidia.conf /etc/X11/xorg.conf.d/nvidia.conf
                        sed -i '10i\\\tOption "PrimaryGPU" "yes"' /etc/X11/xorg.conf.d/nvidia.conf
                    "#,
                )
                .status()
                .wrap_err("fail to run `sh`")?;
            if !p.success() {
                return Err(eyre!("fail to set nvidia as primary gpu")
                    .note(format!("exit code from sh: {:?}", p.code())));
            }
        }

        Ok(())
    }

    fn check_boardcom_wifi() -> bool {
        std::process::Command::new("sh")
            .arg("-c")
            .arg("lspci | grep -q -i Network | grep -q -i Broadcom")
            .status()
            .is_ok_and(|s| s.success())
    }
    fn check_boardcom_bluetooth() -> bool {
        std::process::Command::new("sh")
            .arg("-c")
            .arg("lspci | grep -q -i Bluetooth| grep -q -i Broadcom")
            .status()
            .is_ok_and(|s| s.success())
    }

    fn setup_broadcom() -> color_eyre::Result<()> {
        if Self::check_boardcom_wifi() {
            tracing::info!("Setting up broadcom wifi drivers");
            let p = std::process::Command::new("dnf")
                .args(["in", "-y", "broadcom-wl", "akmod-wl"])
                .status()
                .wrap_err("fail to run `dnf`")?;
            if !p.success() {
                return Err(eyre!("fail to install broadcom wifi drivers")
                    .note(format!("exit code from dnf: {:?}", p.code())));
            }
        }
        if Self::check_boardcom_bluetooth() {
            tracing::info!("Setting up broadcom bluetooth drivers");
            let p = std::process::Command::new("dnf")
                .args(["in", "-y", "broadcom-bt-firmware"])
                .status()
                .wrap_err("fail to run `dnf`")?;
            if !p.success() {
                return Err(eyre!("fail to install broadcom bluetooth drivers")
                    .note(format!("exit code from dnf: {:?}", p.code())));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_nvidia_driver() {
        assert_eq!(Drivers::get_nvidia_driver("NV34"), "unsupported");
        assert_eq!(Drivers::get_nvidia_driver("GK104"), "470xx");
        assert_eq!(Drivers::get_nvidia_driver("GP108"), "latest");
        assert_eq!(Drivers::get_nvidia_driver("GK208"), "470xx");
        assert_eq!(Drivers::get_nvidia_driver("GT218"), "340xx");
    }
}
