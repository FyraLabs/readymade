use cleanup_boot::CleanupBoot;
use color_eyre::Result;
use dracut::Dracut;
use drivers::Drivers;
use efi_stub::EfiStub;
use enum_dispatch::enum_dispatch;
use grub2::GRUB2;
use initial_setup::InitialSetup;
use prepare_fedora::PrepareFedora;
use reinstall_kernel::ReinstallKernel;
use selinux::SELinux;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod cleanup_boot;
pub mod dracut;
pub mod drivers;
pub mod efi_stub;
pub mod grub2;
pub mod initial_setup;
pub mod prepare_fedora;
pub mod reinstall_kernel;
pub mod selinux;

pub struct Context {
    /// The disk to install to
    pub destination_disk: PathBuf,
    /// Whether the installation is UEFI
    pub uefi: bool,
    /// ESP partition path
    pub esp_partition: Option<String>,
    /// Bootloader partition path
    pub boot_partition: Option<String>,
}

#[enum_dispatch(Module)]
pub trait PostInstallModule {
    fn run(&self, context: &Context) -> Result<()>;
}

#[enum_dispatch]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "module")]
pub enum Module {
    SELinux,
    Dracut,
    ReinstallKernel,
    GRUB2,
    CleanupBoot,
    PrepareFedora,
    Drivers,
    EfiStub,
    InitialSetup,
}
