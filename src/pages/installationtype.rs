use std::sync::LazyLock;

use crate::cfg::InstallationType;
use crate::prelude::*;
use crate::ui::{InstallKind, InstallationTypeState};

static TPM_SUPPORT: LazyLock<bool> = LazyLock::new(|| {
    std::fs::read_to_string("/sys/class/tpm/tpm0/tpm_version_major")
        .ok()
        .and_then(|s: String| s.trim().parse::<usize>().ok())
        .is_some_and(|ver| ver >= 2)
});

const fn installation_type(kind: InstallKind) -> Option<InstallationType> {
    match kind {
        InstallKind::None => None,
        InstallKind::WholeDisk => Some(InstallationType::WholeDisk),
        InstallKind::DualBoot => Some(InstallationType::DualBoot(0)),
        InstallKind::Custom => Some(InstallationType::Custom),
        InstallKind::Chromebook => Some(InstallationType::ChromebookInstall),
    }
}

pub fn init(ui: &AppWindow) {
    let state = ui.global::<InstallationTypeState<'_>>();
    let allowed = &crate::CONFIG.read().install.allowed_installtypes;
    state.set_allow_whole_disk(allowed.contains(&InstallationType::WholeDisk));
    state.set_allow_dual_boot(
        allowed
            .iter()
            .any(|x| matches!(x, InstallationType::DualBoot(_))),
    );
    state.set_allow_custom(allowed.contains(&InstallationType::Custom));
    state.set_allow_chromebook(allowed.contains(&InstallationType::ChromebookInstall));
    state.set_tpm_supported(*TPM_SUPPORT);

    let weak = ui.as_weak();
    state.on_commit(move |passphrase| {
        let Some(ui) = weak.upgrade() else { return };
        let state = ui.global::<InstallationTypeState<'_>>();
        let encrypt = state.get_encrypt() && state.get_can_encrypt();
        let mut app = APPLICATION_STATE.write();
        app.installation_type = installation_type(state.get_kind());
        app.encrypt = encrypt;
        app.tpm = encrypt && state.get_tpm() && *TPM_SUPPORT;
        app.encryption_key = encrypt.then(|| passphrase.to_string());
    });
}
