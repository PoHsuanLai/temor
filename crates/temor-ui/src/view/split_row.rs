//! A tab with several panes, drawn as Arc draws a split in its sidebar: one row cut into equal
//! segments, a segment per pane in the order the panes stand (the hub lists them so), each with the terminal glyph and
//! the pane's name, a hairline between them. The pane with the keys is the segment drawn in ink.
//! A pane where an agent works or waits shows the companion's orb in place of its glyph, so the
//! row says which pane wants the person.
//! Pressing a segment focuses its pane; dragging one out of the sidebar tears the pane off into a
//! tab of its own; a right click is the tab's menu with Close Pane for that pane alone. Nothing
//! closes on hover, as in mailo. A split of three or more panes is squeezed as Arc's is: the pane
//! with the keys keeps its name, the others are their glyphs alone.

use dioxus::prelude::*;
use ds::prelude::IconSize;
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::workspace::WorkspaceIn;

use super::dnd::Dnd;
use super::row_tip::RowTip;
use super::tab_glyph::presence_face;
use super::tab_wiring::{drag_press, selected_attr, tab_menu_trigger};
use crate::hub::{Desk, DragKey, Tile};

/// The glyph's size in a segment: the row's own leading size.
const GLYPH: IconSize = IconSize::Small;

/// The segments of a tab with more than one pane.
#[component]
#[allow(non_snake_case)]
pub fn SplitSegments(
    tab: TabId,
    tiles: Vec<Tile>,
    desk: Desk,
    dnd: Dnd,
    onmenu: EventHandler<PaneId>,
) -> Element {
    let squeezed = match tiles.len() > SIDE_BY_SIDE {
        true => Squeeze::Squeezed,
        false => Squeeze::Named,
    };
    rsx! {
        div { class: "temor-split", role: "group", "aria-label": "Panes in this tab",
            for (index, tile) in tiles.into_iter().enumerate() {
                if index > 0 {
                    span { key: "rule{tile.pane.0}", class: "temor-seg-rule", aria_hidden: "true" }
                }
                {segment(tile, squeezed, desk.clone(), dnd.clone(), onmenu)}
            }
        }
    }
}

/// How many panes stand side by side with their names; more are squeezed.
const SIDE_BY_SIDE: usize = 2;

/// Whether the panes other than the one with the keys show their names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Squeeze {
    Named,
    Squeezed,
}

fn segment(
    tile: Tile,
    squeeze: Squeeze,
    desk: Desk,
    dnd: Dnd,
    onmenu: EventHandler<PaneId>,
) -> Element {
    let pane = tile.pane;
    let selected = selected_attr(tile.selection);
    let glyph_only = (squeeze == Squeeze::Squeezed && selected.is_none()).then_some("true");
    let click = dnd.click.clone();
    // A right click on a segment is the tab's menu, with Close Pane for this pane alone.
    let opening = tab_menu_trigger(move || onmenu.call(pane));
    let mut press = drag_press(&dnd, DragKey::Tile(pane));
    rsx! {
        div { key: "{pane.0}", class: "temor-seg-slot", "data-glyph": glyph_only,
            RowTip { name: format!("pane{}", pane.0), text: tile.title.clone(),
                div {
                    class: "temor-seg",
                    role: "button",
                    "aria-label": tile.title.clone(),
                    "data-selected": selected,
                    onpointerdown: move |event: PointerEvent| {
                        // A press on a segment is the segment's: the row under it starts no drag.
                        event.stop_propagation();
                        press(event);
                    },
                    onclick: move |event: MouseEvent| {
                        event.stop_propagation();
                        click.counted(|| desk.act(WorkspaceIn::Focus(pane)));
                    },
                    oncontextmenu: opening,
                    span { class: "temor-seg-glyph", {presence_face(tile.agent, GLYPH)} }
                    span { class: "temor-seg-name ds-mono", "{tile.title}" }
                }
            }
        }
    }
}
