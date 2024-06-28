use crate::{NavigationAction, INSTALLATION_STATE};
use gettextrs::gettext;
use libhelium::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug, Default)]
pub struct InstallationPage {
    progress_bar: gtk::ProgressBar,
    thread: Option<std::thread::JoinHandle<color_eyre::Result<()>>>,
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

        ComponentParts { model, widgets }
    }

    #[tracing::instrument]
    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        if let Some(th) = &self.thread {
            self.progress_bar.pulse();
            if th.is_finished() {
                let th = self.thread.take().unwrap();
                let res = th.join().expect("Cannot join thread");
                if let Err(e) = res {
                    tracing::error!("Installation failed: {e:?}");
                }
                self.progress_bar.set_fraction(1.0);
            }
        }
        // handle channel logics here
        match message {
            InstallationPageMsg::StartInstallation => {
                self.thread = Some(std::thread::spawn(|| {
                    let state = INSTALLATION_STATE.read();
                    tracing::debug!(?state, "Starting installation...");
                    state.installation_type.as_ref().unwrap().install(&state)?;
                    Ok(())
                }));
            }
            InstallationPageMsg::Navigate(action) => sender
                .output(InstallationPageOutput::Navigate(action))
                .unwrap(),
            InstallationPageMsg::Update => {}
        }
    }
}
