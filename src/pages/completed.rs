use crate::NavigationAction;
use gettextrs::gettext;
use libhelium::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug, Default)]
pub struct CompletedPage {}

#[derive(Debug)]
pub enum CompletedPageMsg {}

#[derive(Debug)]
pub enum CompletedPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for CompletedPage {
    type Init = ();
    type Input = CompletedPageMsg;
    type Output = CompletedPageOutput;

    view! {
        libhelium::ViewMono {
            set_title: &gettext("Completed"),
            set_vexpand: true,

            add = &gtk::Box {

            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {}
}
