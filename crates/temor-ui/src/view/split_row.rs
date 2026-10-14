//! A tab with several panes, drawn as Arc draws a split in its sidebar: one row cut into equal
//! segments, a segment per pane in the order the panes stand, each with the terminal glyph, the
//! pane's name and its own close button, a hairline between them. The pane with the keys is the
//! segment drawn in ink. Pressing a segment focuses its pane; dragging one out of the sidebar
//! tears the pane off into a tab of its own; its × closes that pane alone.

use dioxus::prelude::*;
use ds::prelude::{Icon, IconSize, IconSource, IconView, Selection};
use temor_core::workspace::WorkspaceIn;

use super::dnd::{Click, Dnd, is_primary, point_of};
use super::row_tip::RowTip;
use super::tab_glyph::prompt_glyph;
use crate::hub::{Desk, DragKey, Tile};

/// The glyph's size in a segment: the row's own leading size.
const GLYPH: IconSize = IconSize::Small;

/// The segments of a tab with more than one pane.
#[component]
#[allow(non_snake_case)]
pub fn SplitSegments(tiles: Vec<Tile>, desk: Desk, dnd: Dnd) -> Element {
    rsx! {
        div { class: "temor-split", role: "group", "aria-label": "Panes in this tab",
            for (index, tile) in in_reading_order(tiles).into_iter().enumerate() {
                if index > 0 {
                    span { key: "rule{tile.pane.0}", class: "temor-seg-rule", aria_hidden: "true" }
                }
                {segment(tile, desk.clone(), dnd.clone())}
            }
        }
    }
}

/// The panes as they stand: left to right, then top to bottom.
pub fn in_reading_order(mut tiles: Vec<Tile>) -> Vec<Tile> {
    tiles.sort_by(|a, b| {
        (a.left, a.top)
            .partial_cmp(&(b.left, b.top))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tiles
}

fn segment(tile: Tile, desk: Desk, dnd: Dnd) -> Element {
    let pane = tile.pane;
    let selected = match tile.selection {
        Selection::Selected => Some("true"),
        Selection::Unselected => None,
    };
    let click = dnd.click.clone();
    let closing = desk.clone();
    rsx! {
        div { key: "{pane.0}", class: "temor-seg-slot",
            RowTip { name: format!("pane{}", pane.0), text: tile.title.clone(),
                div {
                    class: "temor-seg",
                    role: "button",
                    "aria-label": tile.title.clone(),
                    "data-selected": selected,
                    onpointerdown: move |event: PointerEvent| {
                        // A press on a segment is the segment's: the row under it starts no drag.
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
                    span { class: "temor-seg-glyph",
                        IconView { source: prompt_glyph(GLYPH), size: GLYPH }
                    }
                    span { class: "temor-seg-name ds-mono", "{tile.title}" }
                    span {
                        class: "temor-seg-close",
                        role: "button",
                        "aria-label": "Close Pane",
                        onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                            closing.act(WorkspaceIn::ClosePane(Some(pane)));
                        },
                        IconView { source: IconSource::Glyph(Icon::X), size: IconSize::Micro }
                    }
                }
            }
        }
    }
}
