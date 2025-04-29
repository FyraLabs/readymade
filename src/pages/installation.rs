use crate::backend::install::InstallationMessage;
use crate::prelude::*;
use crate::{NavigationAction, INSTALLATION_STATE};
use color_eyre::Result;
use l10n::BENTO_LOADER as L;
use relm4::{Component, ComponentParts, ComponentSender};
use std::time::Duration;

mod l10n {
    use const_format::formatcp;
    use i18n_embed::fluent::{fluent_language_loader, FluentLanguageLoader};
    use i18n_embed::{unic_langid::LanguageIdentifier, FileSystemAssets, LanguageLoader as _};
    use itertools::Itertools;
    use std::str::FromStr;
    use std::sync::{Arc, LazyLock};

    /// NOTE: This is sed-ed by the install.sh script!
    #[cfg(not(debug_assertions))]
    const BENTO_ASSETS_PATH: &str = "/usr/share/readymade/bento/";

    #[cfg(debug_assertions)]
    const BENTO_ASSETS_PATH: &str = "";

    type B = Box<dyn i18n_embed::I18nAssets + Send + Sync>;

    static BENTO_ASSETS: LazyLock<Arc<B>> = LazyLock::new(|| {
        Arc::new(
            FileSystemAssets::try_new(formatcp!("{BENTO_ASSETS_PATH}po/"))
                .inspect_err(|e| tracing::error!(?e, "Cannot load assets in {BENTO_ASSETS_PATH}"))
                .inspect_err(|_| tracing::warn!("Falling back to global compile-time assets"))
                .map_or_else(
                    |_| Box::new(crate::Localizations) as B,
                    |a| Box::new(a) as B,
                ),
        )
    });

    static BENTO_AVAILABLE_LANGS: LazyLock<Vec<LanguageIdentifier>> = LazyLock::new(|| {
        fluent_language_loader!()
            .available_languages(&***BENTO_ASSETS)
            .unwrap()
    });

    // WARN: this is written under the assumption that the language is not changed.
    // This assumption is true as long as Readymade doesn't handle the language selections.
    pub(super) static BENTO_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
        let loader = fluent_language_loader!();
        let mut langs = ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE", "LANGUAGES"]
            .into_iter()
            .flat_map(|env| {
                std::env::var(env).ok().into_iter().flat_map(|locales| {
                    locales
                        .split(':')
                        .filter_map(|locale| LanguageIdentifier::from_str(locale).ok())
                        .collect_vec()
                })
            })
            .flat_map(|li| crate::LOCALE_SOLVER.solve_locale(li))
            .filter(|li| BENTO_AVAILABLE_LANGS.contains(li))
            .collect_vec();
        if langs.is_empty() {
            langs = vec![loader.fallback_language().clone()];
        }
        loader.load_languages(&***BENTO_ASSETS, &langs).unwrap();
        loader
    });
}

#[relm4::widget_template(pub)]
impl WidgetTemplate for BentoCard {
    view! {
        gtk::Button {
            set_vexpand: true,
            set_hexpand: true,
            add_css_class: "installation-bento-card",

            gtk::Box {
                set_spacing: 4,
                set_halign: gtk::Align::Fill,
                set_valign: gtk::Align::End,
                set_vexpand: true,

                add_css_class: "content-block",
                inline_css: "border-top-left-radius: 0px; border-top-right-radius: 0px;",

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 16,
                    set_hexpand: true,
                    set_halign: gtk::Align::Start,

                    #[name = "icon"]
                    gtk::Image {
                        set_halign: gtk::Align::Start,
                        set_icon_name: Some("dialog-question-symbolic"),
                        inline_css: "-gtk-icon-size: 28px",
                    },
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        #[name = "title"]
                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            inline_css: "font-weight: 600; font-size: 16px"
                        },
                        #[name = "description"]
                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            inline_css: "font-weight: normal; font-size: 14px"
                        }
                    }
                },
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct InstallationPage {
    progress_bar: gtk::ProgressBar,
}

#[derive(Debug)]
pub enum InstallationPageMsg {
    Open(String),
    StartInstallation,
    #[doc(hidden)]
    Navigate(NavigationAction),
    Update,
    #[doc(hidden)]
    Throb,
    #[doc(hidden)]
    SubprocessMessage(InstallationMessage),
}

#[derive(Debug)]
pub enum InstallationPageCommandMsg {
    FinishInstallation(Result<()>),
    None,
}

#[derive(Debug)]
pub enum InstallationPageOutput {
    Navigate(NavigationAction),
    SendErr(String),
}

#[relm4::component(pub)]
impl Component for InstallationPage {
    type Init = ();
    type Input = InstallationPageMsg;
    type Output = InstallationPageOutput;
    type CommandOutput = InstallationPageCommandMsg;

    view! {
        libhelium::ViewMono {
            // no close btn
            set_show_right_title_buttons: false,
            #[wrap(Some)]
            set_title = &gtk::Label {
                #[watch]
                set_label: &t!("page-installation"),
                set_css_classes: &["view-title"]
            },
            set_vexpand: true,

            append = &gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 16,

                gtk::Grid {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_row_spacing: 8,
                    set_column_spacing: 8,

                    #[template]
                    attach[0, 0, 1, 4] = &BentoCard {
                        connect_clicked => InstallationPageMsg::Open(crate::CONFIG.read().bentos[0].link.clone()),
                        add_css_class: "welcome-card",

                        #[template_child]
                        icon {
                            set_icon_name: Some(&crate::CONFIG.read().bentos[0].icon),
                        },
                        #[template_child]
                        title {
                            set_label: &L.get_args(&crate::CONFIG.read().bentos[0].title, [("distro", &crate::CONFIG.read().distro.name)].into()),
                        },
                        #[template_child]
                        description {
                            set_label: &L.get(&crate::CONFIG.read().bentos[0].desc),
                        }
                    },
                    #[template]
                    attach[1, 0, 1, 2] = &BentoCard {
                        connect_clicked => InstallationPageMsg::Open(crate::CONFIG.read().bentos[1].link.clone()),
                        add_css_class: "help-card",

                        #[template_child]
                        icon {
                            set_icon_name: Some(&crate::CONFIG.read().bentos[1].icon),
                        },
                        #[template_child]
                        title {
                            set_label: &L.get_args(&crate::CONFIG.read().bentos[1].title, [("distro", &crate::CONFIG.read().distro.name)].into()),
                        },
                        #[template_child]
                        description {
                            set_label: &L.get(&crate::CONFIG.read().bentos[1].desc),
                        }
                    },
                    #[template]
                    attach[1, 2, 1, 2] = &BentoCard {
                        connect_clicked => InstallationPageMsg::Open(crate::CONFIG.read().bentos[2].link.clone()),
                        add_css_class: "contribute-card",

                        #[template_child]
                        icon {
                            set_icon_name: Some(&crate::CONFIG.read().bentos[2].icon),
                        },
                        #[template_child]
                        title {
                            set_label: &L.get_args(&crate::CONFIG.read().bentos[2].title, [("distro", &crate::CONFIG.read().distro.name)].into()),
                        },
                        #[template_child]
                        description {
                            set_label: &L.get(&crate::CONFIG.read().bentos[2].desc),
                        }
                    },
                    // #[template]
                    // attach[1, 3, 1, 1] = &BentoCard {
                    //     connect_clicked => InstallationPageMsg::Open("https://github.com/sponsors/FyraLabs".to_string()),
                    //     add_css_class: "sponsor-card",

                    //     #[template_child]
                    //     icon {
                    //         set_icon_name: Some("power-profile-power-saver-symbolic"),
                    //     },
                    //     #[template_child]
                    //     title {
                    //         set_label: &gettext("Sponsor Fyra Labs"),
                    //     },
                    //     #[template_child]
                    //     description {
                    //         set_label: &gettext("Sponsorships help us ship better software, faster."),
                    //     }
                    // },
                },

                #[local_ref]
                progress_bar -> gtk::ProgressBar {
                    set_show_text: true
                }
            }
        }
    }

    fn init(
        _init: Self::Init, // TODO: use selection state saved in root
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self::default();
        let progress_bar = &model.progress_bar;
        progress_bar.set_text(Some(&t!("page-installation-progress")));

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallationPageMsg::Update);

        gtk::glib::timeout_add(Duration::from_secs(1), move || {
            sender.input(InstallationPageMsg::Throb);
            gtk::glib::ControlFlow::Continue
        }); // TODO: cleanup

        ComponentParts { model, widgets }
    }

    #[tracing::instrument]
    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, widget: &Self::Root) {
        match message {
            InstallationPageMsg::Open(uri) => gtk::UriLauncher::new(&uri).launch(
                widget.toplevel_window().as_ref(),
                gtk::gio::Cancellable::NONE,
                |_| {},
            ),
            InstallationPageMsg::StartInstallation => {
                let sender2 = sender.clone();
                let (s, r) = relm4::channel();
                sender.oneshot_command(async move {
                    r.forward(sender2.input_sender().clone(), |msg| {
                        InstallationPageMsg::SubprocessMessage(msg)
                    })
                    .await;

                    InstallationPageCommandMsg::None
                });

                sender.spawn_oneshot_command(move || {
                    let state = crate::backend::install::FinalInstallationState::from(
                        &*INSTALLATION_STATE.read(),
                    );
                    tracing::debug!(?state, "Starting installation...");

                    InstallationPageCommandMsg::FinishInstallation(
                        state.install_using_subprocess(&s),
                    )
                });
            }
            InstallationPageMsg::Navigate(action) => sender
                .output(InstallationPageOutput::Navigate(action))
                .unwrap(),
            InstallationPageMsg::Update => {}
            InstallationPageMsg::Throb => self.progress_bar.pulse(),
            InstallationPageMsg::SubprocessMessage(InstallationMessage::Status(status)) => {
                self.progress_bar.set_text(Some(&status));
            }
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        match message {
            InstallationPageCommandMsg::FinishInstallation(res) => {
                if let Err(e) = res {
                    sender
                        .output(InstallationPageOutput::SendErr(format!("{e:?}")))
                        .unwrap();
                    sender
                        .output(InstallationPageOutput::Navigate(NavigationAction::GoTo(
                            crate::Page::Failure,
                        )))
                        .unwrap();
                } else {
                    sender
                        .output(InstallationPageOutput::Navigate(NavigationAction::GoTo(
                            crate::Page::Completed,
                        )))
                        .unwrap();
                }
            }
            InstallationPageCommandMsg::None => {}
        }
    }
}
