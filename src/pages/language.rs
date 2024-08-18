use crate::prelude::*;
use crate::NavigationAction;
use relm4::RelmIterChildrenExt;
use relm4::{ComponentParts, RelmWidgetExt, SharedState, SimpleComponent};
use std::rc::Rc;

static SEARCH_STATE: SharedState<gtk::glib::GString> = SharedState::new();
// This is a list of languages sorted by total speakers:
// https://en.wikipedia.org/wiki/List_of_languages_by_total_number_of_speakers
// (2024-08-17)
//
// These are filtered by our Ultramarine website plausible statistics and the 5 most popular
// langauges in the world.
const POPULAR_LANGS: [&str; 9] = [
    "en_US", "zh_CN", "zh_TW", "hi_IN", "es_ES", "ar_AE", "fr_FR", "pt_BR", "de_DE",
];

#[derive(Debug)]
struct LanguageRow {
    locale: String,
    name: String,
    native_name: String,
}

impl From<(String, (String, String))> for LanguageRow {
    fn from(value: (String, (String, String))) -> Self {
        let (locale, (name, native_name)) = value;
        Self {
            locale,
            name,
            native_name,
        }
    }
}
impl From<LanguageRow> for (String, (String, String)) {
    fn from(val: LanguageRow) -> Self {
        (val.locale, (val.name, val.native_name))
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
    btnfactory: Rc<relm4::factory::FactoryVecDeque<LanguageRow>>,
    search: libhelium::TextField,
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
                set_vexpand: true,

                #[local_ref]
                search -> libhelium::TextField {
                    set_is_search: true,
                    set_is_outline: true,
                    set_margin_top: 6,
                    set_margin_bottom: 6,
                    set_prefix_icon: Some("system-search-symbolic"),
                    set_placeholder_text: Some(&gettext("Search Language/Locale…")),
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

                    libhelium::Button {
                        set_is_pill: true,
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

        // sort the popular languages, put to top
        for lang in POPULAR_LANGS.iter().rev() {
            let Some(index) = btnfactory
                .iter()
                .position(|l: &LanguageRow| l.locale.starts_with(lang))
            else {
                continue;
            };
            let Some(x) = btnfactory.guard().remove(index) else {
                unreachable!()
            };
            btnfactory.guard().push_front(x.into());
        }

        let model = Self {
            btnfactory: Rc::new(btnfactory),
            search: libhelium::TextField::new(),
        };
        let btnbox = model.btnfactory.widget();
        let btnfactory2 = Rc::clone(&model.btnfactory);
        model.search.internal_entry().connect_changed(move |en| {
            *SEARCH_STATE.write() = en.text();
            btnfactory2.widget().invalidate_filter();
            tracing::trace!(?en, "Search Changed!");
        });
        let btnfactory = Rc::clone(&model.btnfactory);
        btnbox.set_filter_func(move |row| {
            let s = SEARCH_STATE.read().as_str().to_ascii_lowercase();
            #[allow(clippy::cast_sign_loss)]
            let lang = btnfactory.get(row.index() as usize).unwrap();
            lang.locale.to_ascii_lowercase().starts_with(&s)
                || lang.native_name.to_ascii_lowercase().contains(&s)
                || lang.name.to_ascii_lowercase().starts_with(&s)
        });
        let search = &model.search;
        let widgets = view_output!();

        // autoselect en_US (first entry)
        let btnfactory = Rc::clone(&model.btnfactory);
        btnfactory
            .widget()
            .select_row(btnfactory.widget().iter_children().next().as_ref());

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
