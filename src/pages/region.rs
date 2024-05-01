use relm4::SimpleComponent;

pub struct RegionPage {}

// #[derive(Debug)]
// pub enum RegionPageMsg {
//     #[doc(hidden)]
//     Navigate(NavigationAction),
// }

#[relm4::component(pub)]
impl SimpleComponent for RegionPage {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        libhelium::ViewMono {
            set_title: &gettext("Region"),
            set_vexpand: true,
            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                // TODO: ??
            }
        }
    }
}
