use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use he::subclass::prelude::*;

mod imp {
    use gdk::prelude::StaticTypeExt;

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(file = "src/window.blp")]
    pub struct ApplicationWindow {
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ApplicationWindow {
        const NAME: &'static str = "ApplicationWindow";
        type Type = super::ApplicationWindow;
        type ParentType = he::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            crate::views::welcome::WelcomePage::ensure_type();
            klass.bind_template();
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ApplicationWindow {
        fn constructed(&self) {
            self.parent_constructed();
        }
    }
    impl WidgetImpl for ApplicationWindow {}
    impl WindowImpl for ApplicationWindow {}
    impl HeWindowImpl for ApplicationWindow {}

    impl ApplicationWindowImpl for ApplicationWindow {}
    impl HeApplicationWindowImpl for ApplicationWindow {}
}

glib::wrapper! {
  pub struct ApplicationWindow(ObjectSubclass<imp::ApplicationWindow>)
      @extends gtk::Widget, gtk::Window, he::Window, gtk::ApplicationWindow, he::ApplicationWindow,
      @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
}

impl ApplicationWindow {
    pub fn new(app: &he::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
