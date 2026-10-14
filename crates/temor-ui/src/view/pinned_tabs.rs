//! The pinned tabs at the top of the sidebar, as Arc's and mailo's pinned tiles are: square tiles
//! four to a row, each the tab's glyph (the companion's orb while an agent works or waits) with
//! its name on hover and a dot when it has news. Pressing a tile brings its tab to the front; its
//! menu unpins it. A tile drags as Arc's do (`dnd::resolve`): onto another tile to take its place,
//! onto a row to be unpinned there; a row dropped on a tile is pinned there.
//!
//! quire's `PinTiles` draws accounts (an initial, a colour, a provider mark); a terminal tab is a
//! glyph, so the tile is temor's own, in `PinTile`'s plate and sizes.

use dioxus::prelude::*;
use ds::base::vocab::CompanionPresence;
use ds::components::companion::orb::model::OrbSize;
use ds::components::companion::orb::view::CompanionOrb;
use ds::components::controls::badge::{Badge, BadgeContent, BadgeTone};
use ds::host::measure::use_rect;
use ds::prelude::{IconSize, IconView, Selection, Shown};
use temor_core::presence::companion_presence;
use temor_core::workspace::{TabSel, TabsIn, WorkspaceIn};

use super::dnd::{Click, Dnd, is_primary, point_of};
use super::radar::{AgentTabMenu, PinAct};
use super::row_tip::RowTip;
use super::tab_glyph::prompt_glyph;
use crate::hub::{Desk, DragKey, TabRow, Unread};

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
    let rect = use_rect();
    // The drag finds the tile under the pointer as it finds a row.
    if let Some(at) = rect.rect() {
        dnd.rects.set(id, at);
    }
    {
        let rects = dnd.rects.clone();
        use_drop(move || rects.forget(id));
    }
    let over = ((dnd.over)() == Some(id)).then_some("true");
    let click = dnd.click.clone();
    let mut menu = use_signal(|| Shown::Hidden);
    let selected = match row.selection {
        Selection::Selected => Some("true"),
        Selection::Unselected => None,
    };
    let face = match companion_presence(row.agent) {
        CompanionPresence::Idle => rsx! {
            IconView { source: prompt_glyph(IconSize::Large), size: IconSize::Large }
        },
        presence => rsx! {
            CompanionOrb { presence, size: OrbSize::Inline }
        },
    };
    let (picking, opening) = (desk.clone(), desk.clone());
    rsx! {
        RowTip { name: format!("pin{}", id.0), text: row.title.clone(),
            div {
                class: "temor-pin",
                role: "button",
                "aria-label": row.title.clone(),
                "data-selected": selected,
                "data-over": over,
                onmounted: move |event: MountedEvent| rect.on_mounted(event),
                onpointerdown: move |event: PointerEvent| {
                    if is_primary(&event) {
                        dnd.click.reset();
                        dnd.drag.down(DragKey::Tab(id), point_of(&event));
                    }
                },
                onclick: move |_| {
                    if click.take() == Click::Counts {
                        picking.act(WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(id))));
                    }
                },
                oncontextmenu: move |event: MouseEvent| {
                    event.prevent_default();
                    opening.agent_menu_opened(id);
                    menu.set(Shown::Visible);
                },
                {face}
                if row.unread == Unread::Unread {
                    span { class: "temor-pin-news",
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
                desk: desk.clone(),
                onclose: move |()| menu.set(Shown::Hidden),
            }
        }
    }
}
