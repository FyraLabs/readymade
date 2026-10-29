pub(crate) use crate::state::APPLICATION_STATE;
pub(crate) use crate::ui::{AppWindow, Page};
pub use color_eyre::eyre::{Context, ContextCompat, OptionExt, WrapErr, bail, eyre};
pub use color_eyre::{Result, Section};
pub use itertools::Itertools;
pub use slint::{ComponentHandle as _, Model as _, ToSharedString as _};

macro_rules! t {
    ($msgid:literal $($tt:tt)*) => {
        i18n_embed_fl::fl!($crate::LL.read(), $msgid $($tt)*)
    };
}

pub(crate) use t;

/// Turn an iterator into a model that can be handed to a Slint property.
pub fn model<T: Clone + 'static>(items: impl IntoIterator<Item = T>) -> slint::ModelRc<T> {
    slint::ModelRc::new(slint::VecModel::from(items.into_iter().collect_vec()))
}
