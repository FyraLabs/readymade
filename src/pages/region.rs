use gettextrs::gettext;
use libhelium::prelude::*;
use relm4::{ComponentParts, SimpleComponent};

use crate::NavigationAction;

// Model
pub struct RegionPage {}

#[derive(Debug)]
pub enum RegionPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum RegionPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for RegionPage {
    type Input = RegionPageMsg;
    type Output = RegionPageOutput;
    type Init = ();

    view! {
        libhelium::ViewMono {
            set_title: &gettext("Region"),
            set_vexpand: true,
            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                // TODO: ??
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: relm4::prelude::ComponentSender<Self>,
    ) -> relm4::prelude::ComponentParts<Self> {
        let model = RegionPage {};
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: relm4::prelude::ComponentSender<Self>) {
        match message {
            RegionPageMsg::Navigate(action) => {
                sender.output(RegionPageOutput::Navigate(action)).unwrap()
            }
        }
    }
}
