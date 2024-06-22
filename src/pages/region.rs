use gettextrs::gettext;
use libhelium::prelude::*;
use relm4::gtk::prelude::ButtonExt;
use relm4::{ComponentParts, RelmWidgetExt, SimpleComponent};

use crate::NavigationAction;

#[derive(Debug)]
struct RegionButton {
    timezone_name: String,
}

impl From<&str> for RegionButton {
    fn from(value: &str) -> Self {
        Self {
            timezone_name: value.into(),
        }
    }
}

#[relm4::factory]
impl relm4::factory::FactoryComponent for RegionButton {
    type Init = String;
    type Input = ();
    type Output = relm4::factory::DynamicIndex;
    type CommandOutput = ();
    type ParentWidget = relm4::gtk::FlowBox;

    // todo: use listboxrow and listbox for region and city selection
    // move click connection to the parent listbox, use index to get timezone
    view! {
        #[root]
        gtk::Button {
            set_label: &self.timezone_name,
            connect_clicked[sender, index] => move |_| {
                sender.output(index.clone()).unwrap();
            }
        }
    }

    fn init_model(
        value: Self::Init,
        _index: &relm4::factory::DynamicIndex,
        _sender: relm4::FactorySender<Self>,
    ) -> Self {
        Self {
            timezone_name: value,
        }
    }
}

// Model
pub struct RegionPage {
    regionfactory: relm4::factory::FactoryVecDeque<RegionButton>,
    cityfactory: relm4::factory::FactoryVecDeque<RegionButton>,
    timezones: std::collections::HashMap<String, Vec<&'static str>>,
}

#[derive(Debug)]
pub enum RegionPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
    // Selected(relm4::factory::DynamicIndex),
    RegionSelectionChanged,
    CitySelectionChanged,
    // Mouse clicked
    RegionClick(relm4::factory::DynamicIndex),
    CityClick(relm4::factory::DynamicIndex),
}

#[derive(Debug)]
pub enum RegionPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for RegionPage {
    type Input = RegionPageMsg;
    type Output = RegionPageOutput;
    type Init = ();

    view! {
        libhelium::ViewMono {
            set_title: &gettext("Region"),
            set_vexpand: true,
            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    gtk::ScrolledWindow {
                        #[local_ref]
                        btnbox -> gtk::FlowBox {
                            set_selection_mode: gtk::SelectionMode::Single,
                            set_orientation: gtk::Orientation::Horizontal,
                            set_vexpand: true,
                            set_hexpand: true,
                            set_valign: gtk::Align::Center,
                            set_halign: gtk::Align::Center,
                            set_min_children_per_line: 1,
                            set_max_children_per_line: 1,
                            set_column_spacing: 4,
                            set_row_spacing: 4,
                            connect_selected_children_changed => RegionPageMsg::RegionSelectionChanged,
                        }
                    },
                    gtk::ScrolledWindow {
                        #[local_ref]
                        citybtnbox -> gtk::FlowBox {
                            set_selection_mode: gtk::SelectionMode::Single,
                            set_orientation: gtk::Orientation::Horizontal,
                            set_vexpand: true,
                            set_hexpand: true,
                            set_valign: gtk::Align::Center,
                            set_halign: gtk::Align::Center,
                            set_min_children_per_line: 1,
                            set_max_children_per_line: 1,
                            set_column_spacing: 4,
                            set_row_spacing: 4,
                            connect_selected_children_changed => RegionPageMsg::CitySelectionChanged,
                        }
                    },
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    // libhelium::TextButton {
                    //     set_label: &gettext("Previous"),
                    //     connect_clicked => RegionPageMsg::Navigate(NavigationAction::GoTo(crate::Page::Welcome)),
                    // },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::PillButton {
                        set_label: &gettext("Next"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => RegionPageMsg::Navigate(NavigationAction::GoTo(crate::Page::Language)),
                        #[watch]
                        set_sensitive: crate::INSTALLATION_STATE.read().timezone.is_some()
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: relm4::prelude::ComponentSender<Self>,
    ) -> relm4::prelude::ComponentParts<Self> {
        let mut regionfactory = relm4::factory::FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.input_sender(), |output| {
                RegionPageMsg::RegionClick(output)
            });
        let cityfactory = relm4::factory::FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.input_sender(), |output| {
                RegionPageMsg::CityClick(output)
            });

        let mut regionbtns = regionfactory.guard();
        let mut timezones = std::collections::HashMap::new();
        crate::backend::l10n::list_timezones()
            .into_iter()
            .for_each(|tz| {
                if let Some((region, _)) = tz.split_once('/') {
                    let Some(cities) = timezones.get_mut(region) else {
                        regionbtns.push_back(region.into());
                        timezones.insert(region.into(), vec![tz]);
                        return;
                    };
                    cities.push(tz);
                    return;
                }
                regionbtns.push_back(tz.into());
            });
        drop(regionbtns);

        let model = RegionPage {
            regionfactory,
            cityfactory,
            timezones,
        };
        let btnbox = model.regionfactory.widget();
        let citybtnbox = model.cityfactory.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: relm4::prelude::ComponentSender<Self>) {
        match message {
            RegionPageMsg::Navigate(action) => {
                sender.output(RegionPageOutput::Navigate(action)).unwrap()
            }
            RegionPageMsg::RegionSelectionChanged => {
                let regions = self.regionfactory.widget().selected_children();
                let i = regions.first().unwrap().index().try_into().unwrap();
                let region = self.regionfactory.get(i).unwrap();
                if let Some(cities) = self.timezones.get(&region.timezone_name) {
                    // the region contains cities ⇒ repopulate citybtnbox
                    let mut citybtns = self.cityfactory.guard();
                    citybtns.clear();
                    cities.into_iter().for_each(|city| {
                        let (_, city) = city.split_once('/').take().unwrap();
                        let city = city.replace('_', " ");
                        _ = citybtns.push_back(city)
                    });
                    crate::INSTALLATION_STATE.write().timezone = None;
                } else {
                    // just set the timezone (stuff like `UTC` doesn't have cities)
                    crate::INSTALLATION_STATE.write().timezone = Some(region.timezone_name.clone());
                }
                tracing::debug!(?region, "RegionPageMsg::RegionSelectionChanged");
            }
            RegionPageMsg::CitySelectionChanged => {
                let regions = self.regionfactory.widget().selected_children();
                let i = regions.first().unwrap().index().try_into().unwrap();
                let region = self.regionfactory.get(i).unwrap();
                let cities = self.cityfactory.widget().selected_children();
                let i = cities.first().unwrap().index().try_into().unwrap();
                let city = self.cityfactory.get(i).unwrap();
                let tz = self.timezones.get(&region.timezone_name).unwrap().get(i);
                tracing::debug!(?region, ?city, ?tz, "RegionPageMsg::CitySelectionChanged");
                crate::INSTALLATION_STATE.write().timezone = tz.map(ToString::to_string);
            }
            RegionPageMsg::CityClick(index) => self.cityfactory.widget().select_child(
                &self
                    .cityfactory
                    .widget()
                    .child_at_index(index.current_index() as i32)
                    .unwrap(),
            ),
            RegionPageMsg::RegionClick(index) => self.regionfactory.widget().select_child(
                &self
                    .regionfactory
                    .widget()
                    .child_at_index(index.current_index() as i32)
                    .unwrap(),
            ),
        }
    }
}
