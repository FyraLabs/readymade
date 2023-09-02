use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use he::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(file = "src/window.blp")]
    pub struct ApplicationWindow {
        #[template_child]
        pub welcome_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ApplicationWindow {
        const NAME: &'static str = "ApplicationWindow";
        type Type = super::ApplicationWindow;
        type ParentType = he::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
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

            let os = crate::release::release_root("/").unwrap();


            sudo::escalate_if_needed().unwrap();

            self.welcome_label
                .set_text(format!("Welcome to {}", os.pretty_name).as_str());

            // WARN: This requires root privileges
            let devices = distinst::Disks::probe_devices().unwrap();
            // tracing::debug!(dev = ?devices);
            tracing::debug!("physical devices:\n {:#?}", devices);
            devices.physical.iter().for_each(|d| {
                println!("{}: {}", d.model_name, d.device_path.display());
            });
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
