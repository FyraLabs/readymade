#![allow(clippy::module_name_repetitions)]
pub mod completed;
pub mod confirmation;
pub mod destination;
pub mod failure;
pub mod installation;
pub mod installationtype;
pub mod installcustom;
pub mod language;

/// Wire up the callbacks and initial state of every page.
pub fn init(ui: &crate::ui::AppWindow) {
    language::init(ui);
    destination::init(ui);
    installationtype::init(ui);
    installcustom::init(ui);
    confirmation::init(ui);
    installation::init(ui);
    completed::init(ui);
}
