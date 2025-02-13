use cleanup_boot::CleanupBoot;
use color_eyre::Result;
use cryptsetup::CryptSetup;
use dracut::Dracut;
use efi_stub::EfiStub;
use enum_dispatch::enum_dispatch;
use grub2::GRUB2;
use initial_setup::InitialSetup;
use language::Language;
use prepare_fedora::PrepareFedora;
use reinstall_kernel::ReinstallKernel;
use selinux::SELinux;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod cleanup_boot;
pub mod dracut;
pub mod efi_stub;
pub mod grub2;
pub mod initial_setup;
pub mod language;
pub mod prepare_fedora;
pub mod reinstall_kernel;
pub mod selinux;
pub mod cryptsetup;

pub struct Context {
    pub destination_disk: PathBuf,
    pub uefi: bool,
    pub esp_partition: Option<String>,
    // Installs should always have an xbootldr partition
    pub xbootldr_partition: String,
    pub lang: String,
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
    EfiStub,
    InitialSetup,
    Language,
    CryptSetup,
}
