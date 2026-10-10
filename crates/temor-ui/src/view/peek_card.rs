//! The peek card (PLAN §10): anyview's preview of a file, in a small card beside the pointer. It
//! sits over the terminal and lets every press and key through to it, so it never takes the
//! keyboard or the pointer capture. Its size is measured once it is drawn; until then the room it
//! is estimated to take stands in.

use std::sync::Arc;

use anyview_peek::{AnyPeeked, Pane};
use dioxus::prelude::*;
use ds::components::lists::preview::content::PANE_MEDIA;
use ds::host::measure::RectProbe;

/// The card's width (the preview's media box plus the card's padding on both sides) and the
/// height it takes at most, in logical pixels: the room it is placed in until it is measured.
const CARD: (f32, f32) = (PANE_MEDIA.width.0 + 32.0, PANE_MEDIA.height.0 + 140.0);
/// How far the card stands from the pointer.
const GAP: f32 = 14.0;

/// A card to draw, and what it stands beside (logical pixels from the pane's top left).
#[derive(Clone)]
pub struct PeekCard {
    pub peeked: Arc<AnyPeeked>,
    /// Where the pointer was when the card was asked for.
    pub pointer: (f32, f32),
    /// How big the pane is.
    pub room: (f32, f32),
}

impl std::fmt::Debug for PeekCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeekCard")
            .field("pointer", &self.pointer)
            .field("room", &self.room)
            .finish_non_exhaustive()
    }
}

/// Where a card `size` big goes for a pointer at `pointer` in a pane `room` big: right of and below
/// the pointer, above it when there is no room below, and always inside the pane.
pub fn place(pointer: (f32, f32), room: (f32, f32), size: (f32, f32)) -> (f32, f32) {
    let (width, height) = size;
    let left = (pointer.0 + GAP).min(room.0 - width).max(0.0);
    let below = pointer.1 + GAP;
    let top = match below + height > room.1 {
        true => (pointer.1 - GAP - height).max(0.0),
        false => below,
    };
    (left, top)
}

/// The card, in the pane beside the grid. `measure` follows the card's element, so the next
/// placement uses its real size.
pub fn peek_card(card: &PeekCard, measure: RectProbe) -> Element {
    let size = measure
        .rect()
        .map_or(CARD, |rect| (rect.size.width.0, rect.size.height.0));
    let (left, top) = place(card.pointer, card.room, size);
    rsx! {
        div {
            class: "temor-peek",
            onmounted: move |e: MountedEvent| measure.on_mounted(e),
            style: "left:{left}px; top:{top}px; width:{PANE_MEDIA.width.0}px",
            Pane { peeked: card.peeked.clone() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_stands_beside_the_pointer_and_stays_in_the_pane() {
        let (w, h) = CARD;
        let room = (1000.0, 800.0);
        // (pointer, room, expected left, expected top), for a card of the estimated size
        let table = [
            ((100.0, 100.0), room, 100.0 + GAP, 100.0 + GAP),
            // Near the right edge it slides left to fit.
            ((990.0, 100.0), room, room.0 - w, 100.0 + GAP),
            // Near the bottom it goes above the pointer.
            ((100.0, 790.0), room, 100.0 + GAP, 790.0 - GAP - h),
            // In a pane too small for it, it holds to the top left.
            ((10.0, 20.0), (100.0, 100.0), 0.0, 0.0),
        ];
        for (pointer, room, left, top) in table {
            assert_eq!(
                place(pointer, room, CARD),
                (left, top),
                "{pointer:?} in {room:?}"
            );
        }
        // A card measured smaller than the estimate leaves more room: it fits below the pointer.
        assert_eq!(
            place((100.0, 600.0), room, (200.0, 100.0)),
            (100.0 + GAP, 600.0 + GAP)
        );
    }
}
