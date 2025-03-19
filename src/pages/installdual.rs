use crate::prelude::*;

#[derive(Debug)]
struct Model {
    paned: gtk::Paned,
    total_size: u32,
    min_other_allocation: u32,
    min_ultramarine_allocation: u32,
    other_allocation: u32,
    ultramarine_allocation: u32,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            paned: gtk::Paned::new(gtk::Orientation::Horizontal),
            total_size: 500,
            min_other_allocation: 200,
            min_ultramarine_allocation: 200,
            other_allocation: 0,
            ultramarine_allocation: 0,
        }
    }
}

page!(InstallDual {
    inner: Model
}:
    init[paned { model.inner.paned.clone() }](root, sender, model, widgets) {}

    update(self, message, sender) {
        HandleResize => {
            let s = &mut self.inner;
            let slider_percentage = s.paned.position() as f32 / s.paned.width() as f32;
            s.other_allocation =
                (((slider_percentage * s.total_size as f32).round()) as u32).clamp(
                    s.min_other_allocation,
                    s.total_size - s.min_ultramarine_allocation,
                );
            s.ultramarine_allocation = (s.total_size - s.other_allocation)
                .clamp(s.min_ultramarine_allocation, s.total_size);
        }
    } => {}

    #[local_ref]
    paned -> gtk::Paned {
        set_vexpand: true,

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
            set_size_request: ((model.inner.paned.width() as f32 * (model.inner.min_other_allocation as f32 / model.inner.total_size as f32)) as i32, -1),
            gtk::Label {
                #[watch]
                set_label: &t!("page-installdual-otheros"),
            },
            gtk::Label {
                #[watch]
                set_label: &format!("{} GB", model.inner.other_allocation),
            }
        },
        #[wrap(Some)]
        set_end_child = &gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            inline_css: "padding-top: 16px; padding-bottom: 16px;",
            #[watch]
            set_size_request: ((model.inner.paned.width() as f32 * (model.inner.min_ultramarine_allocation as f32 / model.inner.total_size as f32)) as i32, -1),
            gtk::Label {
                #[watch]
                set_label: &crate::CONFIG.read().distro.name,
            },
            gtk::Label {
                #[watch]
                set_label: &format!("{} GB", model.inner.ultramarine_allocation),
            }
        },
    }
);

// how this works:
// - find the partition with the largest size. That partition probably contains the other system
// - resize that partition?
