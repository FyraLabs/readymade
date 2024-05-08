use gettextrs::gettext;
use libhelium::prelude::*;
use relm4::gtk::prelude::ButtonExt;
use relm4::{ComponentParts, RelmWidgetExt, SimpleComponent};

use crate::NavigationAction;

#[derive(Debug)]
struct RegionButton {
    timezone_name: &'static str,
}

impl From<&'static str> for RegionButton {
    fn from(value: &'static str) -> Self {
        Self {
            timezone_name: value,
        }
    }
}

#[relm4::factory]
impl relm4::factory::FactoryComponent for RegionButton {
    type Init = &'static str;
    type Input = ();
    type Output = ();
    type CommandOutput = ();
    type ParentWidget = relm4::gtk::FlowBox;

    view! {
        #[root]
        gtk::Button {
            set_label: self.timezone_name,
        }
    }

    fn init_model(
        value: Self::Init,
        _index: &relm4::factory::DynamicIndex,
        _sender: relm4::FactorySender<Self>,
    ) -> Self {
        Self {
            timezone_name: value,
        }
    }
}

// Model
pub struct RegionPage {
    btnfactory: relm4::factory::FactoryVecDeque<RegionButton>,
}

#[derive(Debug)]
pub enum RegionPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
    // Selected(relm4::factory::DynamicIndex),
    SelectionChanged,
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
                set_spacing: 4,
                gtk::ScrolledWindow {
                    #[local_ref]
                    btnbox -> gtk::FlowBox {
                        set_selection_mode: gtk::SelectionMode::Single,
                        set_orientation: gtk::Orientation::Horizontal,
                        set_vexpand: true,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,
                        set_min_children_per_line: 7,
                        set_max_children_per_line: 7,
                        set_column_spacing: 4,
                        set_row_spacing: 4,
                        connect_selected_children_changed => RegionPageMsg::SelectionChanged,
                    }
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    // libhelium::TextButton {
                    //     set_label: &gettext("Previous"),
                    //     connect_clicked => RegionPageMsg::Navigate(NavigationAction::GoTo(crate::Page::Welcome)),
                    // },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::PillButton {
                        set_label: &gettext("Next"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => RegionPageMsg::Navigate(NavigationAction::GoTo(crate::Page::Language)),
                        #[watch]
                        set_sensitive: crate::INSTALLATION_STATE.read().timezone.is_some()
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: relm4::prelude::ComponentSender<Self>,
    ) -> relm4::prelude::ComponentParts<Self> {
        let mut btnfactory = relm4::factory::FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.input_sender(), |_output| todo!());

        let mut btns = btnfactory.guard();
        crate::backend::l10n::list_timezones()
            .into_iter()
            .for_each(|tz| _ = btns.push_front(tz.into()));
        drop(btns);

        let model = RegionPage { btnfactory };
        let btnbox = model.btnfactory.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: relm4::prelude::ComponentSender<Self>) {
        match message {
            RegionPageMsg::Navigate(action) => {
                sender.output(RegionPageOutput::Navigate(action)).unwrap()
            }
            RegionPageMsg::SelectionChanged => {
                let regions = self.btnfactory.widget().selected_children();
                let i = regions.first().unwrap().index().try_into().unwrap();
                let region = self.btnfactory.get(i).unwrap();
                crate::INSTALLATION_STATE.write().timezone = Some(region.timezone_name);
            }
        }
    }
}
