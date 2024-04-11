mod albius;
mod pages;
mod util;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt};
use relm4::{gtk, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent};

// todo: lazy_static const variables for the setup params

// todo: GtkStack for paging

// todo: wizard

// the code is non-existent, but the boilerplate is there

const APPID: &str = "com.fyralabs.Readymade";

struct AppModel {
    counter: u8,
}

#[derive(Debug)]
enum AppMsg {
    Increment,
    Decrement,
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = u8;

    type Input = AppMsg;
    type Output = ();

    view! {
        gtk::Window {
            set_title: Some("Readymade Installer"),
            set_default_width: 300,
            set_default_height: 100,




            gtk::Box {

                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,
                set_margin_all: 5,

                #[template]
                crate::pages::welcome::Welcome {
                    // distro_name: "Ultramarine Linux".to_string(),
                },


                // gtk::Label {
                //     set_label: "Welcome to Readymade Installer!",
                // },
                // // insert logo here i guess, branding time

                // gtk::Button {
                //     set_label: "Increment",
                //     connect_clicked => AppMsg::Increment
                // },

                // gtk::Button::with_label("Decrement") {
                //     connect_clicked => AppMsg::Decrement
                // },

                // gtk::Label {
                //     #[watch]
                //     set_label: &format!("Counter: {}", model.counter),
                //     set_margin_all: 5,
                // }
            }
        }
    }

    // Initialize the UI.
    fn init(
        counter: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AppModel { counter };

        // Insert the macro code generation here
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Increment => {
                self.counter = self.counter.wrapping_add(1);
            }
            AppMsg::Decrement => {
                self.counter = self.counter.wrapping_sub(1);
            }
        }
    }
}

fn main() {
    let app = RelmApp::new(APPID);
    app.run::<AppModel>(0);
}
