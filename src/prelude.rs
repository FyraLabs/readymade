pub(crate) use crate::{NavigationAction, Page, INSTALLATION_STATE};
pub use color_eyre::eyre::{bail, eyre};
pub use color_eyre::eyre::{Context, ContextCompat, OptionExt as _};
pub use color_eyre::{Result, Section};
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
        tracing::debug!("page initialised");
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

// pub(crate) use kurage_generated_macros::kurage_page_pre;
macro_rules! t {
    ($msgid:literal $($tt:tt)*) => {
        i18n_embed_fl::fl!($crate::LL.read(), $msgid $($tt)*)
    };
}

macro_rules! t_expr {
    ($msgid:expr$(, $($tt:tt)*)?) => {
        paste::paste! { with_builtin_macros::with_builtin!(let $id = $msgid in {
            i18n_embed_fl::fl!($crate::LL.read(), $id$(, $($tt)*)?)
        })}
    };
}

pub(crate) use {page, t, t_expr};
