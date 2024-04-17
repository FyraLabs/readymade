use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

pub struct DestinationPage {}

#[relm4::component(pub)]
impl SimpleComponent for DestinationPage {
    type Input = ();
    type Init = ();
    type Output = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 5,

            // insert logo here i guess, branding time
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }
}
