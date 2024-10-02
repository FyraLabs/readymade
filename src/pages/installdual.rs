use crate::{prelude::*, NavigationAction};

pub struct InstallDualPage;

#[derive(Debug)]
pub enum InstallDualPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum InstallDualPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallDualPage {
    type Init = ();
    type Input = InstallDualPageMsg;
    type Output = InstallDualPageOutput;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                set_label: &gettext("Dual Boot"),
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
            InstallDualPageMsg::Navigate(action) => sender
                .output(InstallDualPageOutput::Navigate(action))
                .unwrap(),
        }
    }
}

// how this works:
// - find the partition with the largest size. That partition probably contains the other system
// - resize that partition?
