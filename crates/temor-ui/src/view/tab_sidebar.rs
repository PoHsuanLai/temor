//! The sidebar (PLAN §7): the tabs as quire's `Sidebar`, pinned tabs first, then each folder's
//! tabs under its name, then the rest. The sections' order is the model's (`hub::rows`), so a row
//! and its ⌘1 to ⌘9 key always name the same tab. The sidebar is drawn in the Space's frame ink
//! (`use_space_frame`), so a Space's look reaches it as it does quire's own sidebars.

use dioxus::prelude::*;
use ds::base::press::Press;
use ds::components::chrome::sidebar::Sidebar;
use ds::components::chrome::sidebar_model::{SidebarFill, SidebarSection};
use ds::components::content::icon_source::IconSource;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_core::vocab::ids::TabId;
use temor_core::workspace::{TabSel, TabsIn, WorkspaceIn};

use super::dnd::Dnd;
use super::shelf::ShelfFoot;
use super::tab_row::TabRowView;
use crate::hub::{Desk, SidebarData, TabRow};

/// The window's sidebar of tabs.
#[component]
#[allow(non_snake_case)]
pub fn TabSidebar(data: SidebarData, desk: Desk, dnd: Dnd) -> Element {
    let frame = use_space_frame();
    let mut sections = Vec::new();
    group(&mut sections, "Pinned", &data.pinned, &desk, &dnd);
    for (title, rows) in &data.folders {
        group(&mut sections, title, rows, &desk, &dnd);
    }
    // The rest need a name only when something else is named.
    let named = !data.pinned.is_empty() || data.folders.iter().any(|(_, rows)| !rows.is_empty());
    match named {
        true => group(&mut sections, "Tabs", &data.loose, &desk, &dnd),
        false => rows_of(&mut sections, &data.loose, &desk, &dnd),
    }
    let cursor = [&data.pinned, &data.loose]
        .into_iter()
        .chain(data.folders.iter().map(|(_, rows)| rows))
        .flatten()
        .find(|row| row.selection == Selection::Selected)
        .map(|row| row.id);
    let select = {
        let desk = desk.clone();
        EventHandler::new(move |tab: TabId| {
            desk.act(WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(tab))));
        })
    };
    let new_tab = {
        let desk = desk.clone();
        move |_: Press| desk.act(WorkspaceIn::NewTab { cwd: None })
    };
    rsx! {
        div { class: "temor-side", style: frame.style_attr(),
            Sidebar::<TabId> {
                label: "Tabs",
                sections,
                cursor,
                onselect: select,
                fill: SidebarFill::Clear,
                foot: rsx! {
                    ShelfFoot { desk: desk.clone() }
                    Button {
                        bezel: Bezel::Toolbar,
                        image: ImagePosition::Only,
                        size: ControlSize::Small,
                        icon: Some(IconSource::Glyph(Icon::Plus)),
                        label: "New tab".to_owned(),
                        title: Some("New tab".to_owned()),
                        onclick: new_tab,
                    }
                },
            }
        }
    }
}

/// A named group: its heading, then its rows. A group with no rows is left out.
fn group(
    sections: &mut Vec<SidebarSection<TabId>>,
    title: &str,
    rows: &[TabRow],
    desk: &Desk,
    dnd: &Dnd,
) {
    if rows.is_empty() {
        return;
    }
    sections.push(SidebarSection::Custom(rsx! {
        SectionHeader { title: title.to_owned() }
    }));
    rows_of(sections, rows, desk, dnd);
}

/// A list of tab rows.
fn rows_of(sections: &mut Vec<SidebarSection<TabId>>, rows: &[TabRow], desk: &Desk, dnd: &Dnd) {
    if rows.is_empty() {
        return;
    }
    let items = rows
        .iter()
        .map(|row| {
            let view = rsx! {
                TabRowView { row: row.clone(), desk: desk.clone(), dnd: dnd.clone() }
            };
            ListItem::row(row.id, row.title.clone(), view)
        })
        .collect();
    sections.push(SidebarSection::List(items));
}
