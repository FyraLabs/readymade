pub(crate) use crate::{NavigationAction, Page, INSTALLATION_STATE};
pub use itertools::Itertools;
pub use libhelium::{glib::prelude::*, prelude::*};
pub use relm4::{
    gtk::{self, prelude::*},
    prelude::*,
};

kurage::kurage_gen_macros!();
kurage::generate_generator! { page => [<$name Page>]
    init: {
        INSTALLATION_STATE.subscribe($sender.input_sender(), |_| Self::Input::Update);
    }

    update: {
        Navigate(action: NavigationAction) => $sender.output([<$name PageOutput>]::Navigate(action)).unwrap(),
        Update => {},
    } => { Navigate(NavigationAction), }

    libhelium::ViewMono {
        #[wrap(Some)]
        set_title = &gtk::Label {
            #[watch]
            set_label: &t_expr!(concat!("page-", stringify!([<$name:lower>]))),
            set_css_classes: &["view-title"],
        },
        set_vexpand: true,

        append = &gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 4,
            set_margin_all: 16,
            set_vexpand: true,
            set_hexpand: true,

            KURAGE_INNER
        },
    },
}

macro_rules! pagename {
    () => {{
        let s = std::path::Path::new(file!())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap();
        &format!(
            "{}{}",
            s.chars().nth(0).unwrap().to_ascii_uppercase(),
            &s[1..s.len() - 3]
        )
    }};
}

// pub(crate) use kurage_generated_macros::kurage_page_pre;
macro_rules! t {
    ($msgid:literal $($tt:tt)*) => {
        i18n_embed_fl::fl!($crate::LL.read().as_ref().unwrap(), $msgid $($tt)*)
    };
}

macro_rules! t_expr {
    ($msgid:expr$(, $($tt:tt)*)?) => {
        paste::paste! { with_builtin_macros::with_builtin!(let $id = $msgid in {
            i18n_embed_fl::fl!($crate::LL.read().as_ref().unwrap(), $id$(, $($tt)*)?)
        })}
    };
}

pub(crate) use {page, pagename, t, t_expr};
