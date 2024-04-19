use crate::NavigationAction;
use gtk::prelude::*;
use libhelium::prelude::*;
use relm4::{
    factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque},
    ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, WidgetTemplate,
};

pub struct InstallationPage {}

#[derive(Debug)]
pub enum InstallationPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum InstallationPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallationPage {
    type Init = ();
    type Input = InstallationPageMsg;
    type Output = InstallationPageOutput;

    view! {
        libhelium::ViewMono {
            set_title: "Destination",
            set_vexpand: true,
            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                gtk::Box {
                    set_spacing: 8,
                    set_halign: gtk::Align::Center,
                    set_homogeneous: true,
                    libhelium::PillButton {
                        set_label: "Erase & Install",
                        inline_css: "padding-left: 48px; padding-right: 48px"
                    },
                    libhelium::PillButton {
                        set_label: "Dual Boot",
                        inline_css: "padding-left: 48px; padding-right: 48px"

                    },
                    libhelium::PillButton {
                        set_label: "Custom",
                        inline_css: "padding-left: 48px; padding-right: 48px",
                    }
                },
            }
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

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            InstallationPageMsg::Navigate(action) => sender
                .output(InstallationPageOutput::Navigate(action))
                .unwrap(),
        }
    }
}
