//! The window's content: a sidebar of the four pages beside the page shown, which scrolls.

use dioxus::prelude::*;
use ds::components::chrome::sidebar::Sidebar;
use ds::components::chrome::sidebar_model::SidebarSection;
use ds::prelude::*;
use temor_panes::pane::keys::SettingsPane;

use super::general::SchemaPage;
use super::keyboard::KeyboardPage;
use super::open::TITLE;
use super::spaces::SpacesPage;

/// The icon a page has in the sidebar.
fn icon(page: SettingsPane) -> Icon {
    match page {
        SettingsPane::General => Icon::Settings,
        SettingsPane::Appearance => Icon::SunMoon,
        SettingsPane::Keyboard => Icon::Keyboard,
        SettingsPane::Spaces => Icon::Grid,
    }
}

/// The sidebar of pages beside the page `page` names; a click on a page turns to it.
#[component]
#[allow(non_snake_case)]
pub fn SettingsView(page: Signal<SettingsPane>) -> Element {
    let shown = page();
    let go = move |to: SettingsPane| {
        let mut page = page;
        page.set(to);
    };
    let items: Vec<ListItem<SettingsPane>> = SettingsPane::ALL
        .into_iter()
        .map(|each| {
            let selection = if each == shown {
                Selection::Selected
            } else {
                Selection::Unselected
            };
            ListItem::row(
                each,
                each.name(),
                rsx! {
                    Row {
                        title: TextLine::from(each.name()),
                        leading: RowLeading::Icon(icon(each)),
                        state: ds::base::vocab::RowState { selection, ..Default::default() },
                        common: ds::root::common::Common {
                            aria_label: Some(each.name().to_owned()),
                            ..Default::default()
                        },
                        onclick: move |_| go(each),
                    }
                },
            )
        })
        .collect();
    rsx! {
        div { class: "temor-settings",
            Sidebar::<SettingsPane> {
                label: TITLE,
                sections: vec![SidebarSection::List(items)],
                cursor: Some(shown),
                onselect: move |next: SettingsPane| go(next),
            }
            div { class: "temor-settings-scroll temor-settings-page", "data-page": shown.name(),
                match shown {
                    SettingsPane::General | SettingsPane::Appearance => rsx! {
                        SchemaPage { key: "{shown.name()}", pane: shown }
                    },
                    SettingsPane::Keyboard => rsx! { KeyboardPage {} },
                    SettingsPane::Spaces => rsx! { SpacesPage {} },
                }
            }
        }
    }
}
