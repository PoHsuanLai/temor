//! The sidebar (PLAN §7), laid out as mailo's and Arc's are: the Space's name and Downloads at the
//! top, the Favourites as tiles, the pinned tabs' rows and a hairline under them, each folder's
//! tabs under its name, then New Tab and the rest; at the foot, the Space dots and one chevron that holds everything else. The name and the dots show
//! only once there is a second Space. It is quire's `Sidebar` on the ladder's 32, each row one
//! line. The sections' order is the model's (`hub::rows`), so a row and its ⌘1 to ⌘9 key always
//! name the same tab. The sidebar is drawn in the Space's frame ink (`use_space_frame`), so a
//! Space's look reaches it as it does quire's own sidebars. It shows the tabs of the Space on
//! screen.

use dioxus::prelude::*;
use ds::base::press::{PointerButton, Press};
use ds::components::app::edge_peek::EdgePeek;
use ds::components::chrome::sidebar::Sidebar;
use ds::components::chrome::sidebar_model::{SidebarFill, SidebarSection};
use ds::prelude::*;
use ds::style::tokens::control_size::SidebarSize;
use temor_core::vocab::ids::TabId;
use temor_core::workspace::WorkspaceIn;
use temor_pane::keys::Action;

use super::dnd::Dnd;
use super::favourites::Favourites;
use super::row_tip::RowTip;
use super::space_ui::{SpaceFoot, SpaceHeader, space_menu_on};
use super::tab_row::TabRowView;
use crate::hub::{Desk, SidebarData, TabRow, select_tab};

/// A row of the sidebar: a tab, or Arc's New Tab row above the loose tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SideKey {
    Tab(TabId),
    NewTab,
}

/// The window's sidebar of tabs.
#[component]
#[allow(non_snake_case)]
pub fn TabSidebar(data: SidebarData, desk: Desk, dnd: Dnd, pinned: Shown) -> Element {
    let frame = use_space_frame();
    let mut sections = Vec::new();
    let every: Vec<TabRow> = [&data.pinned, &data.loose]
        .into_iter()
        .chain(data.folders.iter().map(|(_, rows)| rows))
        .flatten()
        .cloned()
        .collect();
    sections.push(SidebarSection::Custom(rsx! {
        Favourites { rows: every.clone(), desk: desk.clone(), dnd: dnd.clone() }
    }));
    // The pinned tabs are rows, as Arc's are, over a hairline that parts them from the rest.
    if !data.pinned.is_empty() {
        sections.push(SidebarSection::List(items_of(
            &data.pinned,
            true,
            &desk,
            &dnd,
        )));
        sections.push(SidebarSection::Custom(rsx! {
            div { class: "temor-side-rule", role: "separator" }
        }));
    }
    for (title, rows) in &data.folders {
        group(&mut sections, title, rows, &desk, &dnd);
    }
    // New Tab heads the loose tabs, as Arc's does; they need a name only when a folder is named.
    if data.folders.iter().any(|(_, rows)| !rows.is_empty()) {
        sections.push(SidebarSection::Custom(rsx! {
            SectionHeader { title: "Tabs".to_owned() }
        }));
    }
    let mut loose = vec![new_tab_row(&desk)];
    loose.extend(items_of(&data.loose, false, &desk, &dnd));
    sections.push(SidebarSection::List(loose));
    let cursor = every
        .iter()
        .find(|row| row.selection == Selection::Selected)
        .map(|row| SideKey::Tab(row.id));
    let select = {
        let desk = desk.clone();
        EventHandler::new(move |key: SideKey| match key {
            SideKey::Tab(tab) => desk.act(select_tab(tab)),
            SideKey::NewTab => desk.act(WorkspaceIn::NewTab { cwd: None }),
        })
    };
    let mut sidebar = desk.sidebar;
    rsx! {
        // Hidden, the sidebar floats over the pane from the window's edge while the pointer is
        // there (quire's `EdgePeek`, as mailo's does), and a click on the edge pins it again.
        EdgePeek {
            label: "Sidebar",
            pinned,
            onpin: move |()| sidebar.set(Shown::Visible),
        div { class: "temor-side", style: frame.style_attr(), oncontextmenu: space_menu_on(&desk),
            Sidebar::<SideKey> {
                label: "Tabs",
                sections,
                cursor,
                onselect: select,
                // The sidebar ladder's 32, mailo's: every row is one line.
                size: SidebarSize::Medium,
                fill: SidebarFill::Flat,
                header: rsx! { SpaceHeader { desk: desk.clone() } },
                // The Space dots; a new Space is in the Space's menu (a right-click on the sidebar).
                foot: rsx! {
                    div { class: "temor-foot",
                    SpaceFoot {
                        desk: desk.clone(),
                        trailing: rsx! {
                            // As wide as the lead, so the dots stand at the centre.
                            div { class: "temor-foot-end" }
                        },
                    }
                    }
                },
            }
        }
        }
    }
}

/// Arc's New Tab row: a plus and the words, laid out as a tab's row is (the list's hover wash and
/// all). A click opens the tab, and so does the list's pick (`onselect`, the keyboard's).
fn new_tab_row(desk: &Desk) -> ListItem<SideKey> {
    let title = Action::NewTab.menu_title();
    let tip = desk.hub.tooltip(title, Action::NewTab);
    let opening = desk.clone();
    let view = rsx! {
        div { class: "temor-new-tab",
            RowTip { name: "new-tab".to_owned(), text: tip,
                Row {
                    leading: RowLeading::Source(IconSource::Glyph(Icon::Plus)),
                    title,
                    // A row's own view takes the press, as a tab's does: the list's pick is the
                    // keyboard's.
                    onclick: move |press: Press| {
                        if !matches!(press.button, PointerButton::Secondary) {
                            opening.act(WorkspaceIn::NewTab { cwd: None });
                        }
                    },
                }
            }
        }
    };
    ListItem::row(SideKey::NewTab, title.to_owned(), view)
}

/// A named group: its heading, then its rows. A group with no rows is left out.
fn group(
    sections: &mut Vec<SidebarSection<SideKey>>,
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
    sections.push(SidebarSection::List(items_of(rows, false, desk, dnd)));
}

/// The rows of a list of tabs, `pinned` or not.
fn items_of(rows: &[TabRow], pinned: bool, desk: &Desk, dnd: &Dnd) -> Vec<ListItem<SideKey>> {
    rows.iter()
        .map(|row| {
            let view = rsx! {
                TabRowView { row: row.clone(), pinned, desk: desk.clone(), dnd: dnd.clone() }
            };
            ListItem::row(SideKey::Tab(row.id), row.title.clone(), view)
        })
        .collect()
}
