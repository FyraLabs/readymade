use std::path::PathBuf;

use relm4::factory::{positions::GridPosition, Position};

use crate::{prelude::*, NavigationAction};

pub struct InstallCustomPage;

#[derive(Debug)]
pub enum InstallCustomPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum InstallCustomPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallCustomPage {
    type Init = ();
    type Input = InstallCustomPageMsg;
    type Output = InstallCustomPageOutput;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                set_label: &gettext("Custom"),
                set_css_classes: &["view-title"],
            },
            set_vexpand: true,
        }
    }

    fn init(
        (): Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            InstallCustomPageMsg::Navigate(action) => sender
                .output(InstallCustomPageOutput::Navigate(action))
                .unwrap(),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct ChooseMount {
    partition: PathBuf,
    mountpoint: PathBuf,
    options: String,
}

#[derive(Debug)]
enum ChooseMountMsg {
    Remove,
}

#[relm4::factory]
impl FactoryComponent for ChooseMount {
    type ParentWidget = gtk::Box;
    type Input = ChooseMountMsg;
    type Output = ();
    type CommandOutput = ();
    type Init = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,

            // FIXME: make this a dropdown?
            libhelium::TextField {
                set_is_outline: true,
                set_margin_top: 6,
                set_margin_bottom: 6,
                set_placeholder_text: &gettext("Partition"),
            },

            libhelium::TextField {
                set_is_outline: true,
                set_margin_top: 6,
                set_margin_bottom: 6,
                set_placeholder_text: &gettext("Mountpoint"),
            },

            libhelium::Button {
                set_icon_name: "gtk-edit",
                set_tooltip: &gettext("Mount options"),
                // TODO: connect_clicked
            }

            libhelium::Button {
                set_icon_name: "delete",
                set_tooltip: &gettext("Remove mountpoint"),
                // TODO: connect_clicked
            }
        }
    }

    fn init_model(_init: Self::Init, _index: &Self::Index, _sender: FactorySender<Self>) -> Self {
        Self::default()
    }
}
