use gettextrs::gettext;
use libhelium::prelude::*;
use relm4::gtk::prelude::ButtonExt;
use relm4::{ComponentParts, RelmWidgetExt, SimpleComponent};

use crate::NavigationAction;

#[derive(Debug)]
struct LanguageButton {
    locale: String,
    name: String,
}

impl From<(String, String)> for LanguageButton {
    fn from(value: (String, String)) -> Self {
        Self {
            locale: value.0,
            name: value.1,
        }
    }
}

#[relm4::factory]
impl relm4::factory::FactoryComponent for LanguageButton {
    type Init = (String, String);
    type Input = ();
    type Output = ();
    type CommandOutput = ();
    type ParentWidget = relm4::gtk::FlowBox;

    view! {
        #[root]
        gtk::Button {
            set_label: &self.name,
        }
    }

    fn init_model(
        value: Self::Init,
        _index: &relm4::factory::DynamicIndex,
        _sender: relm4::FactorySender<Self>,
    ) -> Self {
        Self::from(value)
    }
}

// Model
pub struct LanguagePage {
    btnfactory: relm4::factory::FactoryVecDeque<LanguageButton>,
}

#[derive(Debug)]
pub enum LanguagePageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
    // Selected(relm4::factory::DynamicIndex),
    SelectionChanged,
}

#[derive(Debug)]
pub enum LanguagePageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for LanguagePage {
    type Input = LanguagePageMsg;
    type Output = LanguagePageOutput;
    type Init = ();

    view! {
        libhelium::ViewMono {
            set_title: &gettext("Language"),
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
                        connect_selected_children_changed => LanguagePageMsg::SelectionChanged,
                    }
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    libhelium::TextButton {
                        set_label: &gettext("Previous"),
                        connect_clicked => LanguagePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Region)),
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::PillButton {
                        set_label: &gettext("Next"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => LanguagePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Welcome)),
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
        crate::backend::l10n::list_langs()
            .into_iter()
            .for_each(|x| _ = btns.push_front(x));
        drop(btns);

        let model = LanguagePage { btnfactory };
        let btnbox = model.btnfactory.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: relm4::prelude::ComponentSender<Self>) {
        match message {
            LanguagePageMsg::Navigate(action) => {
                sender.output(LanguagePageOutput::Navigate(action)).unwrap()
            }
            LanguagePageMsg::SelectionChanged => {
                let languages = self.btnfactory.widget().selected_children();
                let i = languages.first().unwrap().index().try_into().unwrap();
                let language = self.btnfactory.get(i).unwrap();
                gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, &*language.locale);
                crate::INSTALLATION_STATE.write().langlocale = Some(language.locale.to_string());
            }
        }
    }
}
