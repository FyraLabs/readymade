use crate::prelude::*;
use crate::{NavigationAction, INSTALLATION_STATE};
use color_eyre::Result;
use relm4::{Component, ComponentParts, ComponentSender};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct InstallationPage {
    progress_bar: gtk::ProgressBar,
}

#[derive(Debug)]
pub enum InstallationPageMsg {
    StartInstallation,
    #[doc(hidden)]
    Navigate(NavigationAction),
    Update,
    #[doc(hidden)]
    Throb,
}

#[derive(Debug)]
pub enum InstallationPageCommandMsg {
    FinishInstallation(Result<()>),
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
            #[wrap(Some)]
            set_title = &gtk::Label {
                set_label: &gettext("Installation"),
                set_css_classes: &["view-title"]
            },
            set_vexpand: true,

            append = &gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                // gtk::Box {
                //     set_vexpand: true,
                //     gtk::Label {
                //         set_label: "Some sort of ad/feature thing here idk."
                //     },
                // },

                gtk::Grid {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_row_spacing: 4,
                    set_column_spacing: 4,

                    attach[0, 0, 1, 1] = &gtk::Box {
                        set_vexpand: true,
                        set_hexpand: true,

                        gtk::Label {
                            set_label: "Meow 1"
                        }
                    },
                    attach[0, 1, 1, 1] = &gtk::Box {
                        set_vexpand: true,
                        set_hexpand: true,

                        gtk::Label {
                            set_label: "Meow 2"
                        }
                    },
                    attach[1, 0, 1, 2] = &gtk::Box {
                        set_vexpand: true,
                        set_hexpand: true,

                        gtk::Label {
                            set_label: "Meow 3"
                        }
                    },
                },

                gtk::Label {
                    #[watch]
                    set_label: &*gettext("Installing base system...")
                },

                #[local_ref]
                progress_bar -> gtk::ProgressBar {}
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

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallationPageMsg::Update);

        gtk::glib::timeout_add(Duration::from_secs(1), move || {
            sender.input(InstallationPageMsg::Throb);
            gtk::glib::ControlFlow::Continue
        }); // TODO: cleanup

        ComponentParts { model, widgets }
    }

    #[tracing::instrument]
    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        match message {
            InstallationPageMsg::StartInstallation => {
                sender.spawn_oneshot_command(|| {
                    let state = INSTALLATION_STATE.read();
                    tracing::debug!(?state, "Starting installation...");
                    InstallationPageCommandMsg::FinishInstallation(state.install_using_subprocess())
                });
            }
            InstallationPageMsg::Navigate(action) => sender
                .output(InstallationPageOutput::Navigate(action))
                .unwrap(),
            InstallationPageMsg::Update => {}
            InstallationPageMsg::Throb => self.progress_bar.pulse(),
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
                tracing::debug!("Installation complete");
                if let Err(e) = res {
                    tracing::error!("Installation failed: {e:?}");
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
        }
    }
}
