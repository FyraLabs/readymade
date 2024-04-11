use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt};
use relm4::{
    gtk, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent, WidgetTemplate,
};

const DISTRO: &str = "Ultramarine Linux";

// create widget for welcome page

// pub struct Welcome;

// impl Default for Welcome {
//     fn default() -> Self {
//         Self {
//             distro_name: DISTRO.to_string(),
//         }
//     }
// }

#[relm4::widget_template(pub)]
impl WidgetTemplate for Welcome {
    view! {

        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 5,
            set_margin_all: 5,

            gtk::Label {
                set_label: &format!("Welcome to {}", DISTRO.to_string()),
            },
            // insert logo here i guess, branding time
        }
    }
}

impl Welcome {
    fn model() -> () {}

    fn update(&mut self, _event: ()) {}

    fn init_view(&mut self) {}
}

