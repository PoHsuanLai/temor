//! The pinned tabs at the top of the sidebar, as Arc's and mailo's pinned tiles are: square tiles
//! four to a row, each the tab's glyph with its name on hover and a dot when it needs the
//! person. Pressing a tile brings its tab to the front; its
//! menu unpins it. A tile drags as Arc's do (`dnd::resolve`): onto another tile to take its place,
//! onto a row to be unpinned there; a row dropped on a tile is pinned there.
//!
//! quire's `PinTiles` draws accounts (an initial, a colour, a provider mark); a terminal tab is a
//! glyph, so the tile is temor's own, in `PinTile`'s plate and sizes.

use dioxus::prelude::*;
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::prelude::{IconSize, IconView, Shown};

use super::dnd::Dnd;
use super::radar::{AgentTabMenu, PinAct};
use super::row_tip::RowTip;
use super::tab_glyph::prompt_glyph;
use super::tab_wiring::{drag_press, selected_attr, tab_menu_trigger, use_drag_target};
use crate::hub::{Desk, DragKey, TabRow, Unread, select_tab};

/// The pinned tabs' tiles.
#[component]
#[allow(non_snake_case)]
pub fn PinnedTabs(rows: Vec<TabRow>, desk: Desk, dnd: Dnd) -> Element {
    rsx! {
        div { class: "temor-pins", role: "group", "aria-label": "Pinned Tabs",
            for row in rows {
                PinTile { key: "{row.id.0}", row, desk: desk.clone(), dnd: dnd.clone() }
            }
        }
    }
}

/// One pinned tab's tile; a secondary press opens its menu (Unpin Tab and the agent's actions).
#[component]
#[allow(non_snake_case)]
fn PinTile(row: TabRow, desk: Desk, dnd: Dnd) -> Element {
    let id = row.id;
    // The drag finds the tile under the pointer as it finds a row.
    let rect = use_drag_target(id, &dnd.rects);
    let over = ((dnd.over)() == Some(id)).then_some("true");
    let click = dnd.click.clone();
    let mut menu = use_signal(|| Shown::Hidden);
    let picking = desk.clone();
    let opening = tab_menu_trigger(move || menu.set(Shown::Visible));
    let press = drag_press(&dnd, DragKey::Tab(id));
    rsx! {
        RowTip { name: format!("pin{}", id.0), text: row.title.clone(),
            div {
                class: "temor-pin",
                role: "button",
                "aria-label": row.title.clone(),
                "data-selected": selected_attr(row.selection),
                "data-over": over,
                onmounted: move |event: MountedEvent| rect.on_mounted(event),
                onpointerdown: press,
                onclick: move |_| {
                    click.counted(|| {
                        picking.act(select_tab(id));
                    });
                },
                oncontextmenu: opening,
                IconView { source: prompt_glyph(IconSize::Large), size: IconSize::Large }
                if row.unread == Unread::Unread {
                    span { class: "temor-pin-news temor-tab-needs",
                        Badge { content: BadgeContent::Dot, tone: BadgeTone::Alert }
                    }
                }
            }
        }
        if let (Shown::Visible, Some(at)) = (menu(), rect.rect()) {
            AgentTabMenu {
                tab: id,
                at,
                pin: PinAct::Unpin,
                close: None,
                desk: desk.clone(),
                onclose: move |()| menu.set(Shown::Hidden),
            }
        }
    }
}
