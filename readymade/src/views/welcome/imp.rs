use gdk::prelude::StaticTypeExt;

use super::*;

#[derive(Debug, Default, gtk::CompositeTemplate)]
#[template(file = "src/views/welcome/view.blp")]
pub struct WelcomePage {
}

#[glib::object_subclass]
impl ObjectSubclass for WelcomePage {
    const NAME: &'static str = "WelcomePage";
    type Type = super::WelcomePage;
    type ParentType = he::Bin;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
        klass.bind_template_callbacks();
    }

    // You must call `Widget`'s `init_template()` within `instance_init()`.
    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for WelcomePage {
    fn constructed(&self) {
        self.parent_constructed();
    }
}
impl WidgetImpl for WelcomePage {}
impl BinImpl for WelcomePage {}

#[gtk::template_callbacks]
impl WelcomePage {}
