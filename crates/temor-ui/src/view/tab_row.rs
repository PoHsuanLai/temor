//! One tab's row in the sidebar (PLAN §7): quire's `Row` on one line, as mailo's and Arc's are,
//! led by what runs in it (its app's icon, its brand, or a letter tile), with the tab's name and, trailing, one dot when it needs the person
//! (news, or an agent that waits). Its folder, and what plugins say of it, are said on hover. A tab
//! with several panes is Arc's split row instead: a segment per pane (`split_row`). Pressing the
//! row brings the tab to the front, and ⌘-pressing it opens a new tab in its folder; dragging it
//! onto another row joins the two tabs.

use dioxus::prelude::*;
use ds::base::press::{PointerButton, Press};
use ds::base::vocab::{Availability, DropState, Emphasis, RowState};
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::components::controls::press::Propagation;
use ds::components::overlays::drag_ghost::DropLine;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::vocab::text::Cwd;
use temor_client::model::workspace::WorkspaceIn;

use super::dnd::{Dnd, Target};
use super::row_menu::RowMenu;
use super::row_tip::RowTip;
use super::split_row::SplitSegments;
use super::tab_glyph::row_leading;
use super::tab_wiring::{drag_press, tab_menu_trigger, use_drag_target};
use super::tool_button::ToolButton;
use crate::hub::{Desk, DragKey, TabRow, Unread, select_tab};

/// The row for one tab.
#[component]
#[allow(non_snake_case)]
pub fn TabRowView(row: TabRow, pinned: bool, desk: Desk, dnd: Dnd) -> Element {
    let id = row.id;
    let mut menu = use_signal(|| Shown::Hidden);
    // The pane whose segment was right-clicked: the menu offers to close it.
    let mut menu_pane = use_signal(|| None::<PaneId>);
    // The drag finds the row under the pointer from where the rows are.
    let rect = use_drag_target(id, &dnd.rects);
    let drop = match (dnd.over)() {
        Some(Target::Tab(over)) if over == id => DropState::Target,
        _ => match dnd.drag.phase() {
            ds::motion::drag::DragPhase::Live {
                key: DragKey::Tab(held),
                ..
            } if held == id => DropState::Source,
            _ => DropState::Idle,
        },
    };
    let emphasis = match row.unread {
        Unread::Unread => Emphasis::Strong,
        Unread::Seen => Emphasis::Plain,
    };
    // What runs in the tab leads the row (`row_leading`); the one dot trails it. The words are
    // the row's own title, so their size is quire's.
    let leading = row_leading(&row.icon);
    let closing = desk.clone();
    let trail = rsx! {
        if row.unread == Unread::Unread {
            span { class: "temor-tab-needs",
                Badge { content: BadgeContent::Dot, tone: BadgeTone::Alert }
            }
        }
        span {
            class: "temor-tab-close",
            "data-selected": (row.selection == Selection::Selected).to_string(),
            ToolButton {
                icon: Icon::X,
                size: ControlSize::Mini,
                propagation: Propagation::Stop,
                label: "Close tab",
                tip: "Close this tab",
                onclick: move |()| closing.act(WorkspaceIn::CloseTab(id)),
            }
        }
    };
    // A split is its segments, each with its own glyph, so the row leads with none.
    let (leading, content) = match row.tiles.is_empty() {
        true => (leading, None),
        false => (
            RowLeading::None,
            Some(rsx! {
                SplitSegments {
                    tab: id,
                    tiles: row.tiles.clone(),
                    desk: desk.clone(),
                    dnd: dnd.clone(),
                    onmenu: move |pane: PaneId| {
                        menu_pane.set(Some(pane));
                        menu.set(Shown::Visible);
                    },
                }
            }),
        ),
    };
    let overlays = rsx! {
        if drop == DropState::Target {
            DropLine {}
        }
        if let (Shown::Visible, Some(at)) = (menu(), rect.rect()) {
            RowMenu {
                tab: id,
                at,
                pinned,
                place: row.place.clone(),
                close: menu_pane(),
                desk: desk.clone(),
                onclose: move |()| {
                    menu.set(Shown::Hidden);
                    menu_pane.set(None);
                },
            }
        }
    };
    let click = dnd.click.clone();
    let selecting = desk.clone();
    let place = row.place.clone();
    let opening = tab_menu_trigger(move || menu.set(Shown::Visible));
    let dragging = drag_press(&dnd, DragKey::Tab(id));
    let tip = hover_text(&row);
    rsx! {
        div {
            class: "temor-tab-box",
            "data-pinned": pinned.then_some("true"),
            oncontextmenu: opening,
            // The folder and what plugins say of it are said on hover: the row is one line.
            RowTip { name: format!("tab{}", id.0), text: tip,
            Row {
                title: row.title.as_str(),
                content,
                leading,
                accessory: Accessory::Slot(trail),
                state: RowState {
                    selection: row.selection,
                    emphasis,
                    availability: Availability::Enabled,
                    drop,
                },
                onclick: move |press: Press| {
                    // A secondary press is the tab's menu (`oncontextmenu` above); it does not
                    // select the tab.
                    if matches!(press.button, PointerButton::Secondary) {
                        return;
                    }
                    let place = place.clone();
                    click.counted(|| match opens_beside(press.modifiers) {
                        true => selecting.act(WorkspaceIn::NewTab { cwd: place.map(Cwd) }),
                        false => selecting.act(select_tab(id)),
                    });
                },
                onpointerdown: dragging,
                common: Common {
                    mounted: Some(EventHandler::new(move |event: MountedEvent| rect.on_mounted(event))),
                    ..Common::default()
                },
            }
            }
            {overlays}
        }
    }
}

/// Whether a press with `modifiers` opens a new tab rather than choosing this one: ⌘ on a Mac,
/// Ctrl elsewhere, as a browser's link is opened beside the page.
fn opens_beside(modifiers: Modifiers) -> bool {
    match cfg!(target_os = "macos") {
        true => modifiers.intersects(Modifiers::SUPER | Modifiers::META),
        false => modifiers.contains(Modifiers::CONTROL),
    }
}

/// What hover says: the tab's folder, then what plugins say of it.
fn hover_text(row: &TabRow) -> String {
    hover_words(row.cwd.as_deref(), row.about.as_deref(), &row.title)
}

/// The hover lines for a tab's `folder` and plugins' `about`; the `title` when neither is known.
fn hover_words(folder: Option<&str>, about: Option<&str>, title: &str) -> String {
    let lines: Vec<&str> = [folder, about].into_iter().flatten().collect();
    match lines.is_empty() {
        true => title.to_owned(),
        false => lines.join("\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_says_the_folder_then_what_plugins_say() {
        let table = [
            ("nothing known", None, None, "zsh"),
            ("folder", Some("~/src"), None, "~/src"),
            ("plugins", None, Some("cluster prod"), "cluster prod"),
            (
                "both",
                Some("~/src"),
                Some("cluster prod"),
                "~/src\ncluster prod",
            ),
        ];
        for (name, folder, about, want) in table {
            assert_eq!(hover_words(folder, about, "zsh"), want, "{name}");
        }
    }
}
