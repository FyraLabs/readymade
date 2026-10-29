use crate::prelude::*;
use crate::ui::FailureState;

/// Show the failure page with the given error.
pub fn show(ui: &AppWindow, err: &str) {
    let failure = ui.global::<FailureState<'_>>();
    let mut log = failure.get_log().to_string();
    log.push_str(&strip_ansi_escapes::strip_str(err));
    failure.set_log(log.into());
    ui.global::<crate::ui::Nav<'_>>().set_page(Page::Failure);
}
