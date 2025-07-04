use crate::prelude::*;
use i18n_embed::LanguageLoader;
use relm4::RelmIterChildrenExt;
use relm4::SharedState;
use std::rc::Rc;

static SEARCH_STATE: SharedState<gtk::glib::GString> = SharedState::new();

#[derive(Clone, Debug)]
struct LanguageRow {
    locale: &'static str,
    name: &'static str,
    native_name: &'static str,
}

taidan_proc_macros::comptime_localedef_langrows!(LANGUAGE_ROWS);

#[relm4::factory]
impl relm4::factory::FactoryComponent for &'static LanguageRow {
    type Widgets = LanguageRowWidgets;
    type Init = &'static LanguageRow;
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
        init: Self::Init,
        _index: &relm4::factory::DynamicIndex,
        _sender: relm4::FactorySender<Self>,
    ) -> Self {
        init
    }
}

#[derive(Debug)]
struct BtnFactory(Rc<relm4::factory::FactoryVecDeque<&'static LanguageRow>>);

impl Default for BtnFactory {
    #[allow(clippy::needless_for_each)]
    fn default() -> Self {
        let mut btnfactory = relm4::factory::FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .detach();

        let mut btns = btnfactory.guard();
        LANGUAGE_ROWS.iter().for_each(|x| _ = btns.push_back(x));
        drop(btns);
        Self(Rc::new(btnfactory))
    }
}

impl std::ops::Deref for BtnFactory {
    type Target = gtk::ListBox;

    fn deref(&self) -> &Self::Target {
        self.0.widget()
    }
}
impl AsRef<gtk::ListBox> for BtnFactory {
    fn as_ref(&self) -> &gtk::ListBox {
        self
    }
}
impl AsRef<gtk::Widget> for BtnFactory {
    fn as_ref(&self) -> &gtk::Widget {
        self.0.widget().upcast_ref()
    }
}

page!(Language {
    btnfactory: BtnFactory,
    search: libhelium::TextField,
}:
    init[search btnfactory { model.btnfactory.0.widget().clone() }](root, sender, model, widgets) {
        let btnfactory2 = btnfactory.clone();
        search.internal_entry().connect_changed(move |en| {
            *SEARCH_STATE.write() = en.text();
            btnfactory2.invalidate_filter();
            tracing::trace!(?en, "Search Changed!");
        });
        let btnfactory3 = model.btnfactory.0.clone();
        btnfactory.set_filter_func(move |row| {
            let s = SEARCH_STATE.read().as_str().to_ascii_lowercase();
            #[allow(clippy::cast_sign_loss)]
            let lang = btnfactory3.get(row.index() as usize).unwrap();
            lang.locale.to_ascii_lowercase().starts_with(&s)
                || lang.native_name.to_ascii_lowercase().contains(&s)
                || lang.name.to_ascii_lowercase().starts_with(&s)
        });
        btnfactory.select_row(btnfactory.iter_children().next().as_ref());
    }

    update(self, message, sender) {
        Selected => {
            if let Some(row) = self.btnfactory.selected_row() {
                #[allow(clippy::cast_sign_loss)]
                let lang = self.btnfactory.0.get(row.index() as usize).unwrap();
                if lang.locale == "en-owo" {
                    let loader = i18n_embed::fluent::fluent_language_loader!();
                    loader
                        .load_languages(&crate::Localizations, &["en-Xowo".parse().unwrap()])
                        .expect("fail to load languages");
                    *crate::LL.write() = loader;
                    crate::INSTALLATION_STATE.write().langlocale = Some("en-US".to_owned());
                } else {
                    set_lang(lang);
                }
            }
        }
    } => {}

    #[local_ref]
    search -> libhelium::TextField {
        set_is_search: true,
        set_is_outline: true,
        set_margin_top: 6,
        set_margin_bottom: 6,
        set_prefix_icon: Some("system-search-symbolic"),
        #[watch]
        set_placeholder_text: Some(&t!("page-language-search-lang")),
    },
    gtk::ScrolledWindow {
        #[local_ref] btnfactory ->
        gtk::ListBox {
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
            set_label: &t!("page-language-next"),
            add_css_class: "large-button",
            connect_clicked => LanguagePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Welcome)),
            #[watch]
            set_sensitive: crate::INSTALLATION_STATE.read().langlocale.is_some()
        }
    }
);

fn set_lang(lang: &LanguageRow) {
    tracing::info!(lang.locale, "Using selected locale");
    if let Ok(locale) = lang
        .locale
        .split_once('.')
        .map_or(&*lang.locale, |(left, _)| left)
        .to_owned()
        .parse::<i18n_embed::unic_langid::LanguageIdentifier>()
        .inspect_err(|e| tracing::error!(?e, "Cannot apply language"))
    {
        let mut locales = crate::LOCALE_SOLVER
            .solve_locale(locale)
            .into_iter()
            .filter(|li| crate::AVAILABLE_LANGS.contains(li))
            .collect_vec();
        let loader = i18n_embed::fluent::fluent_language_loader!();
        if locales.is_empty() {
            locales.push("en-US".parse().unwrap());
        }
        loader
            .load_languages(&crate::Localizations, &locales)
            .expect("fail to load languages");
        tracing::debug!(lang=?loader.current_languages(), welcome=loader.get_args_concrete("page-welcome", std::iter::once(("distro", "Ultramarine Linux".into())).collect()), "new loader");
        *crate::LL.write() = loader;
        crate::INSTALLATION_STATE.write().langlocale = Some(lang.locale.to_owned());
    }
}
