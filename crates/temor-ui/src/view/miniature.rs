//! A tab's split drawn small (PLAN §7): a trailing glyph of one outlined tile per pane, placed
//! where the pane is in the tab. It never changes the row's height. A tile names its pane in a
//! tooltip, focuses it when pressed, and can be dragged out of the sidebar to tear the pane off
//! into a tab of its own.

use dioxus::prelude::*;
use ds::components::overlays::tooltip::Tooltip;
use ds::prelude::Selection;
use temor_core::workspace::WorkspaceIn;

use super::dnd::{Click, Dnd, is_primary, point_of};
use crate::hub::{Desk, DragKey, Tile};

/// The miniature of a tab with more than one pane.
#[component]
#[allow(non_snake_case)]
pub fn Miniature(tiles: Vec<Tile>, desk: Desk, dnd: Dnd) -> Element {
    rsx! {
        div { class: "temor-mini", role: "group", "aria-label": "Panes in this tab",
            for tile in tiles {
                {tile_view(tile, desk.clone(), dnd.clone())}
            }
        }
    }
}

fn tile_view(tile: Tile, desk: Desk, dnd: Dnd) -> Element {
    let pane = tile.pane;
    let place = format!(
        "left:{}%;top:{}%;width:{}%;height:{}%",
        tile.left, tile.top, tile.width, tile.height
    );
    let selected = match tile.selection {
        Selection::Selected => Some("true"),
        Selection::Unselected => None,
    };
    let click = dnd.click.clone();
    rsx! {
        div { key: "{pane.0}", class: "temor-tile-slot", style: place,
            Tooltip { text: tile.title.clone(),
                div {
                    class: "temor-tile",
                    role: "button",
                    "aria-label": tile.title,
                    "data-selected": selected,
                    onpointerdown: move |event: PointerEvent| {
                        // A press on a tile is the tile's: the row under it does not start a drag.
                        event.stop_propagation();
                        if is_primary(&event) {
                            dnd.click.reset();
                            dnd.drag.down(DragKey::Tile(pane), point_of(&event));
                        }
                    },
                    onclick: move |event: MouseEvent| {
                        event.stop_propagation();
                        if click.take() == Click::Counts {
                            desk.act(WorkspaceIn::Focus(pane));
                        }
                    },
                }
            }
        }
    }
}
