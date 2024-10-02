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
