use crate::prelude::*;
use crate::ui::CompletedState;

pub fn init(ui: &AppWindow) {
    ui.global::<CompletedState<'_>>().on_reboot(|| {
        // supposedly it should run pkexec automatically?
        _ = std::process::Command::new("systemctl")
            .arg("reboot")
            .status();
    });
}
