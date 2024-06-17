use crate::{install::run_albius, NavigationAction, INSTALLATION_STATE};
use gettextrs::gettext;
use libhelium::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

pub struct InstallationPage {
    progress: f64,
}

#[derive(Debug)]
pub enum InstallationPageMsg {
    StartInstallation,
    #[doc(hidden)]
    Navigate(NavigationAction),
    Update,
}

#[derive(Debug)]
pub enum InstallationPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallationPage {
    type Init = ();
    type Input = InstallationPageMsg;
    type Output = InstallationPageOutput;

    view! {
        libhelium::ViewMono {
            set_title: &*gettext("Installation"),
            set_vexpand: true,

            add = &gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                gtk::Box {
                    set_vexpand: true,
                    gtk::Label {
                        set_label: "Some sort of ad/feature thing here idk."
                    },
                },

                gtk::Label {
                    set_label: &*gettext("Installing base system...")
                },

                gtk::ProgressBar {
                    #[watch]
                    set_fraction: model.progress
                }
            }
        }
    }

    fn init(
        _init: Self::Init, // TODO: use selection state saved in root
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self { progress: 0.0 };

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallationPageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        // handle channel logics here
        match message {
            InstallationPageMsg::StartInstallation => sender.command(|_out, shutdown| {
                shutdown
                    .register(async move {
                        let state = INSTALLATION_STATE.read();
                        let recipe = crate::install::generate_recipe(&state)?;

                        tracing::debug!(?recipe);

                        run_albius(&recipe)?;

                        color_eyre::Result::<_>::Ok(())
                    })
                    .drop_on_shutdown()
            }),
            InstallationPageMsg::Navigate(action) => sender
                .output(InstallationPageOutput::Navigate(action))
                .unwrap(),
            InstallationPageMsg::Update => {}
        }
    }
}
