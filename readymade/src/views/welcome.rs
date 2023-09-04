use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use he::subclass::prelude::*;

mod imp;

glib::wrapper! {
  pub struct WelcomePage(ObjectSubclass<imp::WelcomePage>)
      @extends gtk::Widget, he::Bin,
      @implements gtk::Accessible, gtk::Actionable,
                  gtk::Buildable, gtk::ConstraintTarget;
}

impl WelcomePage {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
