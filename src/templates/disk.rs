use libhelium::prelude::*;
use relm4::{RelmWidgetExt, WidgetTemplate};

#[relm4::widget_template]
impl WidgetTemplate for MyBox {
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 2,

            gtk::Image {
                set_icon_name: Some("drive-harddisk"),
                inline_css: "-gtk-icon-size: 128px"
            },

            gtk::Label {
                set_label: &self.disk_name,
                inline_css: "font-size: 16px; font-weight: bold"
            },

            gtk::Label {
                set_label: &self.os_name
            }
        }
    }
}
