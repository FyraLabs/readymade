use crate::{prelude::*, NavigationAction};

pub struct InstallDualPage {
    paned: gtk::Paned,
    total_size: u32,
    min_other_allocation: u32,
    min_ultramarine_allocation: u32,
    other_allocation: u32,
    ultramarine_allocation: u32,
}

#[derive(Debug)]
pub enum InstallDualPageMsg {
    HandleResize,
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum InstallDualPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallDualPage {
    type Init = ();
    type Input = InstallDualPageMsg;
    type Output = InstallDualPageOutput;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                set_label: &gettext("Dual Boot"),
                set_css_classes: &["view-title"],
            },
            set_vexpand: true,
            set_hexpand: true,
            append = &gtk::Box {
                #[local_ref]
                paned -> gtk::Paned {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: gtk::Align::Center,
                    set_resize_start_child: true,
                    set_resize_end_child: true,
                    set_shrink_start_child: false,
                    set_shrink_end_child: false,
                    inline_css: "border: 2px solid blue; border-radius: 6px;",
                    connect_position_notify => InstallDualPageMsg::HandleResize,
                    #[wrap(Some)]
                    set_start_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        inline_css: "padding-top: 16px; padding-bottom: 16px;",
                        #[watch]
                        set_size_request: ((model.paned.width() as f32 * (model.min_other_allocation as f32 / model.total_size as f32)) as i32, -1),
                        gtk::Label {
                            set_label: &gettext("Other OS"),
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &format!("{} GB", model.other_allocation),
                        }
                    },
                    #[wrap(Some)]
                    set_end_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        inline_css: "padding-top: 16px; padding-bottom: 16px;",
                        #[watch]
                        set_size_request: ((model.paned.width() as f32 * (model.min_ultramarine_allocation as f32 / model.total_size as f32)) as i32, -1),
                        gtk::Label {
                            set_label: &gettext("Ultramarine"),
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &format!("{} GB", model.ultramarine_allocation),
                        }
                    },
                }
            },
        }
    }

    fn init(
        (): Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            paned: gtk::Paned::new(gtk::Orientation::Horizontal),
            total_size: 500,
            min_other_allocation: 200,
            min_ultramarine_allocation: 200,
            other_allocation: 0,
            ultramarine_allocation: 0,
        };
        let paned = model.paned.clone();

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            InstallDualPageMsg::Navigate(action) => sender
                .output(InstallDualPageOutput::Navigate(action))
                .unwrap(),
            InstallDualPageMsg::HandleResize => {
                let slider_percentage = self.paned.position() as f32 / self.paned.width() as f32;
                self.other_allocation =
                    (((slider_percentage * self.total_size as f32).round()) as u32).clamp(
                        self.min_other_allocation,
                        self.total_size - self.min_ultramarine_allocation,
                    );
                self.ultramarine_allocation = (self.total_size - self.other_allocation)
                    .clamp(self.min_ultramarine_allocation, self.total_size);
            }
        }
    }
}

// how this works:
// - find the partition with the largest size. That partition probably contains the other system
// - resize that partition?
