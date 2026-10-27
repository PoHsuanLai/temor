//! The block list laid out top to bottom, in logical pixels: each entry of the list cut into the
//! segments the window mounts on their own, so a block's long output is a run of tiles and only
//! the tiles near the view are drawn. A card is its head, its tiles and its foot, one under the
//! other with nothing between; the cards have a gap between them. A picture a lens drew for a
//! block goes in its card, under its rows.

use std::ops::Range;
use std::sync::Arc;

use temor_core::block::Outcome;
use temor_core::session::list::{Item, KeptCard, ListBlock};
use temor_core::vocab::ids::BlockId;

use crate::session::{ListMap, ListSpan, Toward};

/// The sizes the layout is made with, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    /// One grid row.
    pub row: f32,
    /// A card's head: its command.
    pub head: f32,
    /// The room under a card's last row, inside it.
    pub foot: f32,
    /// Between one card and the next.
    pub gap: f32,
    /// The least the composer takes.
    pub composer: f32,
    /// The most rows one tile of output holds.
    pub tile_rows: u32,
}

/// How a card stands, for its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Done,
    Failed,
    Running,
}

impl Standing {
    pub fn of(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Running => Standing::Running,
            Outcome::Failed(_) => Standing::Failed,
            _ => Standing::Done,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Standing::Done => "done",
            Standing::Failed => "failed",
            Standing::Running => "running",
        }
    }
}

/// What a segment draws.
#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    /// A command the session ran before it was reopened: a card that is its head alone.
    Kept(KeptCard),
    /// A card's head; `whole` when nothing is under it (no rows, folded): the card is its head.
    Head { block: ListBlock, whole: bool },
    /// Screen rows of a block's output: drawn once as a still, or, `live`, shown from the pane's
    /// own picture as the terminal draws them.
    Rows {
        id: BlockId,
        standing: Standing,
        picked: bool,
        rows: Range<u32>,
        live: bool,
    },
    /// The picture a lens drew for the block, `width` logical pixels across.
    Picture {
        id: BlockId,
        standing: Standing,
        picked: bool,
        width: f32,
        source: Arc<str>,
    },
    Foot {
        id: BlockId,
        standing: Standing,
        picked: bool,
    },
    /// Where the next command is typed: the shell's own prompt rows, live.
    Composer { rows: Range<u32> },
}

/// What identifies a segment across layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SegKey {
    Kept(u32),
    Head(BlockId),
    /// Tile `n` of a block's settled rows.
    Tile(BlockId, u32),
    /// The running block's live rows.
    Live(BlockId),
    Picture(BlockId),
    Foot(BlockId),
    Composer,
}

/// A segment and where it lies.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub key: SegKey,
    pub top: f32,
    pub height: f32,
    pub seg: Seg,
}

impl Placed {
    pub fn bottom(&self) -> f32 {
        self.top + self.height
    }
}

/// The settled rows of a block cut into tiles of `size` rows, each tile's rows counted from the
/// block's first output row so a tile keeps its rows as the block grows.
fn tiles(id: BlockId, rows: Range<u32>, size: u32, first: u32) -> Vec<(SegKey, Range<u32>)> {
    let size = size.max(1);
    if rows.is_empty() {
        return Vec::new();
    }
    let (from, to) = ((rows.start - first) / size, (rows.end - 1 - first) / size);
    (from..=to)
        .map(|n| {
            let start = (first + n * size).max(rows.start);
            let end = (first + (n + 1) * size).min(rows.end);
            (SegKey::Tile(id, n), start..end)
        })
        .collect()
}

/// A lens's picture of a block, at the size it is shown, in logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub block: BlockId,
    pub width: f32,
    pub height: f32,
    /// The picture as a `data:` address.
    pub source: Arc<str>,
}

/// The size a picture `size` device pixels big is shown at, at `factor` device pixels a logical
/// one: its own, shrunk (never enlarged) to `room` across and `most` down.
pub fn picture_size(size: (u32, u32), factor: f32, room: f32, most: f32) -> (f32, f32) {
    let factor = factor.max(f32::EPSILON);
    let (width, height) = (size.0 as f32 / factor, size.1 as f32 / factor);
    let shrink = (room.max(1.0) / width.max(1.0))
        .min(most.max(1.0) / height.max(1.0))
        .min(1.0);
    (width * shrink, height * shrink)
}

/// The list's segments, top to bottom, and the height of the whole list. A block in `pictures`
/// shows its picture under its rows while unfolded.
pub fn lay_out(items: &[Item], sizes: &Sizes, pictures: &[Picture]) -> (Vec<Placed>, f32) {
    let mut out = Vec::new();
    let mut y = 0.0_f32;
    let push = |out: &mut Vec<Placed>, y: &mut f32, key, height, seg| {
        out.push(Placed {
            key,
            top: *y,
            height,
            seg,
        });
        *y += height;
    };
    for item in items {
        match item {
            Item::Kept(card) => {
                let seg = Seg::Kept(card.clone());
                push(&mut out, &mut y, SegKey::Kept(card.n), sizes.head, seg);
                y += sizes.gap;
            }
            Item::Done(block) | Item::Running { block, .. } => {
                let live_from = match item {
                    Item::Running { live_from, .. } => Some(*live_from),
                    _ => None,
                };
                let id = block.id;
                let standing = Standing::of(block.outcome);
                let picked = block.picked;
                let head = out.len();
                push(
                    &mut out,
                    &mut y,
                    SegKey::Head(id),
                    sizes.head,
                    Seg::Head {
                        block: block.clone(),
                        whole: false,
                    },
                );
                if !block.folded {
                    let settled = block.output.start..live_from.unwrap_or(block.output.end);
                    for (key, rows) in tiles(id, settled, sizes.tile_rows, block.output.start) {
                        let height = rows.len() as f32 * sizes.row;
                        let seg = Seg::Rows {
                            id,
                            standing,
                            picked,
                            rows,
                            live: false,
                        };
                        push(&mut out, &mut y, key, height, seg);
                    }
                    if let Some(from) = live_from {
                        let rows = from..block.output.end;
                        if !rows.is_empty() {
                            let height = rows.len() as f32 * sizes.row;
                            let seg = Seg::Rows {
                                id,
                                standing,
                                picked,
                                rows,
                                live: true,
                            };
                            push(&mut out, &mut y, SegKey::Live(id), height, seg);
                        }
                    }
                    if let Some(picture) = pictures.iter().find(|p| p.block == id) {
                        let seg = Seg::Picture {
                            id,
                            standing,
                            picked,
                            width: picture.width,
                            source: Arc::clone(&picture.source),
                        };
                        let key = SegKey::Picture(id);
                        push(&mut out, &mut y, key, picture.height, seg);
                    }
                }
                // A card that is only its head has no room under its last row: the head's own
                // room is the card's, its command in the middle of it.
                let whole = out.len() == head + 1;
                if let Some(Placed {
                    seg: Seg::Head { whole: alone, .. },
                    ..
                }) = out.get_mut(head)
                {
                    *alone = whole;
                }
                let foot = if whole { 0.0 } else { sizes.foot };
                push(
                    &mut out,
                    &mut y,
                    SegKey::Foot(id),
                    foot,
                    Seg::Foot {
                        id,
                        standing,
                        picked,
                    },
                );
                y += sizes.gap;
            }
            Item::Composer { rows } => push(
                &mut out,
                &mut y,
                SegKey::Composer,
                sizes.composer.max(rows.len() as f32 * sizes.row),
                Seg::Composer { rows: rows.clone() },
            ),
        }
    }
    (out, y)
}

/// Where block `id`'s card lies: from its head's top to its foot's bottom.
pub fn card_span(placed: &[Placed], id: BlockId) -> Option<(f32, f32)> {
    let top = placed.iter().find(|p| p.key == SegKey::Head(id))?.top;
    let bottom = placed.iter().find(|p| p.key == SegKey::Foot(id))?.bottom();
    Some((top, bottom))
}

/// A card's head and where the card lies, for the head that stays at the list's top.
#[derive(Debug, Clone, PartialEq)]
pub struct CardPlace {
    pub block: ListBlock,
    pub top: f32,
    pub bottom: f32,
}

/// The cards of a layout, top to bottom.
pub fn card_places(placed: &[Placed]) -> Vec<CardPlace> {
    placed
        .iter()
        .filter_map(|p| match &p.seg {
            Seg::Head { block, .. } => {
                let (top, bottom) = card_span(placed, block.id)?;
                Some(CardPlace {
                    block: block.clone(),
                    top,
                    bottom,
                })
            }
            _ => None,
        })
        .collect()
}

/// The card whose head stays at the list's top with the list scrolled to `offset`, and where
/// that head is drawn, from the view's top: the card whose own head has scrolled away while
/// some of the rest of it shows, its head pushed up by the card's end as that comes near.
pub fn sticky_head(cards: &[CardPlace], offset: f32, head: f32) -> Option<(usize, f32)> {
    let index = cards
        .iter()
        .position(|card| card.top < offset && offset < card.bottom)?;
    let card = &cards[index];
    let pushed = (card.bottom - head - offset).min(0.0);
    Some((index, pushed))
}

/// Where screen row `row` is drawn in the list, `row_height` pixels a row: its top. `None` when
/// no card shows it (folded, or gone from the list).
pub fn row_top(placed: &[Placed], row: u32, row_height: f32) -> Option<f32> {
    placed.iter().find_map(|p| match &p.seg {
        Seg::Rows { rows, .. } | Seg::Composer { rows } if rows.contains(&row) => {
            Some(p.top + (row - rows.start) as f32 * row_height)
        }
        _ => None,
    })
}

/// The segments that lie within `margin` of the view `offset..offset + viewport`: those the
/// window mounts.
pub fn near_view(placed: &[Placed], offset: f32, viewport: f32, margin: f32) -> Range<usize> {
    let (top, bottom) = (offset - margin, offset + viewport + margin);
    let start = placed.partition_point(|p| p.bottom() <= top);
    let end = placed.partition_point(|p| p.top < bottom);
    start..end.max(start)
}

/// The list as the session maps a point over it: each segment's place, its block and its rows.
pub fn list_map(placed: &[Placed], offset: f32) -> ListMap {
    let spans = placed
        .iter()
        .map(|p| {
            let (block, rows, toward) = match &p.seg {
                Seg::Head { block, .. } => (Some(block.id), 0..0, Toward::Next),
                Seg::Rows { id, rows, .. } => (Some(*id), rows.clone(), Toward::Next),
                Seg::Picture { id, .. } | Seg::Foot { id, .. } => (Some(*id), 0..0, Toward::Prev),
                Seg::Composer { rows } => (None, rows.clone(), Toward::Next),
                Seg::Kept(_) => (None, 0..0, Toward::Next),
            };
            ListSpan {
                top: p.top,
                height: p.height,
                block,
                rows,
                toward,
            }
        })
        .collect();
    ListMap { offset, spans }
}

#[cfg(test)]
#[path = "segments_tests.rs"]
mod tests;
