use cleanup_boot::CleanupBoot;
use color_eyre::Result;
use dracut::Dracut;
use enum_dispatch::enum_dispatch;
use grub2::GRUB2;
use prepare_fedora::PrepareFedora;
use reinstall_kernel::ReinstallKernel;
use selinux::SELinux;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod cleanup_boot;
pub mod dracut;
pub mod grub2;
pub mod prepare_fedora;
pub mod reinstall_kernel;
pub mod selinux;

pub struct Context {
    pub destination_disk: PathBuf,
    pub uefi: bool,
}

#[enum_dispatch(Module)]
pub trait PostInstallModule {
    fn run(&self, context: &Context) -> Result<()>;
}

#[enum_dispatch]
#[derive(Serialize, Deserialize, Debug)]
pub enum Module {
    SELinux,
    Dracut,
    ReinstallKernel,
    GRUB2,
    CleanupBoot,
    PrepareFedora,
}
