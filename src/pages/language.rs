use crate::prelude::*;
use relm4::{ComponentParts, RelmWidgetExt, SimpleComponent};

use crate::NavigationAction;

#[derive(Debug)]
struct LanguageRow {
    locale: String,
    name: String,
    native_name: String,
}

impl From<(String, (String, String))> for LanguageRow {
    fn from(value: (String, (String, String))) -> Self {
        Self {
            locale: value.0,
            name: value.1 .0,
            native_name: value.1 .1,
        }
    }
}

#[relm4::factory]
impl relm4::factory::FactoryComponent for LanguageRow {
    type Init = (String, (String, String));
    type Input = ();
    type Output = ();
    type CommandOutput = ();
    type ParentWidget = relm4::gtk::ListBox;

    view! {
        #[root]
        gtk::ListBoxRow {
            libhelium::MiniContentBlock {
                set_title: &self.name,
                set_subtitle: &self.native_name,
            }
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
    btnfactory: relm4::factory::FactoryVecDeque<LanguageRow>,
    search: gtk::Entry,
}

#[derive(Debug)]
pub enum LanguagePageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
    #[doc(hidden)]
    Selected,
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
            #[watch]
            set_title: &gettext("Language"),
            set_vexpand: true,
            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                gtk::SearchBar {
                    // FIXME: … doesn't exist?
                    connect_entry: &model.search
                },
                gtk::ScrolledWindow {
                    #[local_ref]
                    btnbox -> gtk::ListBox {
                        add_css_class: "content-list",
                        set_selection_mode: gtk::SelectionMode::Single,
                        set_vexpand: true,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,
                        connect_selected_rows_changed => LanguagePageMsg::Selected,
                    }
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::PillButton {
                        #[watch]
                        set_label: &gettext("Next"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => LanguagePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Welcome)),
                        #[watch]
                        set_sensitive: crate::INSTALLATION_STATE.read().langlocale.is_some()
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
            .launch(gtk::ListBox::default())
            .detach();

        let mut btns = btnfactory.guard();
        crate::backend::l10n::list_langs()
            .into_iter()
            .sorted_by(|(_, x), (_, y)| x.cmp(y))
            .for_each(|x| _ = btns.push_back(x));
        btns.drop();

        let model = Self {
            btnfactory,
            search: gtk::Entry::new(),
        };
        model.search.connect_changed(|en| println!("{}", en.text()));
        let btnbox = model.btnfactory.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: relm4::prelude::ComponentSender<Self>) {
        match message {
            LanguagePageMsg::Navigate(action) => {
                sender.output(LanguagePageOutput::Navigate(action)).unwrap();
            }
            LanguagePageMsg::Selected => {
                if let Some(row) = self.btnfactory.widget().selected_row() {
                    #[allow(clippy::cast_sign_loss)]
                    let language = self.btnfactory.get(row.index() as usize).unwrap();
                    gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, &*language.locale)
                        .unwrap();
                    crate::INSTALLATION_STATE.write().langlocale = Some(language.locale.clone());
                }
            }
        }
    }
}
