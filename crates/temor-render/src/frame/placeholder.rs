//! Kitty's unicode placeholders: a program places an image with `U=1`, then writes U+10EEEE
//! cells where it should show. Each cell says which slice of the image it holds: the row and
//! column in combining marks after the character, the image id in the foreground colour (its
//! high byte in a third mark), and optionally the placement id in the underline colour.
//!
//! This is the decoding half, ported from libghostty's `graphics_unicode.zig`, which keeps it
//! to itself. Pure, so a table can pin every rule.

// Read only by `FrameCapture` (feature `ghostty`) and tests.
#![cfg_attr(not(feature = "ghostty"), allow(dead_code))]

use temor_vt::{Col, ColorRef, ImageId, PlacementId};

use super::diacritics::diacritic_index;

/// The character a placeholder cell holds. The image is drawn by the placement, so the cell
/// itself draws nothing.
pub(crate) const PLACEHOLDER: char = '\u{10EEEE}';

/// What one placeholder cell says, with the parts it leaves out left open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaceholderCell {
    pub col: Col,
    /// The low 24 bits of the image id, from the foreground colour.
    image_low: u32,
    /// The top 8 bits of the image id, from the third mark.
    image_high: Option<u8>,
    /// 0 when the underline colour is not set.
    placement: u32,
    /// Which row and column of the image this cell holds, from the first two marks.
    row: Option<u32>,
    column: Option<u32>,
}

impl PlaceholderCell {
    /// The cell at `col` holding `text` in these colours, or `None` when `text` is not a
    /// placeholder. A mark that is not one of kitty's numbers counts as absent.
    pub fn decode(col: Col, text: &str, fg: ColorRef, underline: ColorRef) -> Option<Self> {
        let mut chars = text.chars();
        if chars.next() != Some(PLACEHOLDER) {
            return None;
        }
        let mut marks = chars.map(diacritic_index);
        let row = marks.next().flatten();
        let column = marks.next().flatten();
        let image_high = marks.next().flatten().and_then(|v| u8::try_from(v).ok());
        Some(Self {
            col,
            image_low: fg.kitty_id() & 0x00FF_FFFF,
            image_high,
            placement: underline.kitty_id() & 0x00FF_FFFF,
            row,
            column,
        })
    }
}

/// Neighbouring placeholder cells that continue one another: one slice of an image, `width`
/// cells wide and one row tall, starting at `col`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaceholderRun {
    pub col: Col,
    pub width: u32,
    pub image: ImageId,
    /// `PlacementId(0)` when the program gave none.
    pub placement: PlacementId,
    /// The image row and column the run starts at.
    pub row: u32,
    pub column: u32,
}

/// A run being built: the first cell's values, filled in, plus the width so far.
struct Open {
    first: PlaceholderCell,
    row: u32,
    column: u32,
    width: u32,
}

impl Open {
    fn new(first: PlaceholderCell) -> Self {
        Self {
            row: first.row.unwrap_or(0),
            column: first.column.unwrap_or(0),
            width: 1,
            first,
        }
    }

    /// Whether `next` continues the run: the cell right after it, the same image and
    /// placement, and any row, column or high byte it names agrees with the run's.
    fn takes(&self, next: &PlaceholderCell) -> bool {
        let right_after = u32::from(next.col.0) == u32::from(self.first.col.0) + self.width;
        right_after
            && next.image_low == self.first.image_low
            && next.placement == self.first.placement
            && next.row.is_none_or(|r| r == self.row)
            && next.column.is_none_or(|c| c == self.column + self.width)
            && next
                .image_high
                .is_none_or(|h| self.first.image_high == Some(h))
    }

    fn close(self) -> PlaceholderRun {
        let high = u32::from(self.first.image_high.unwrap_or(0));
        PlaceholderRun {
            col: self.first.col,
            width: self.width,
            image: ImageId(self.first.image_low | (high << 24)),
            placement: PlacementId(self.first.placement),
            row: self.row,
            column: self.column,
        }
    }
}

/// The runs of one screen row's placeholder cells (left to right).
pub(crate) fn runs(cells: &[PlaceholderCell]) -> Vec<PlaceholderRun> {
    let mut out = Vec::new();
    let mut open: Option<Open> = None;
    for cell in cells {
        match open.take() {
            Some(mut run) if run.takes(cell) => {
                run.width += 1;
                open = Some(run);
            }
            Some(run) => {
                out.push(run.close());
                open = Some(Open::new(*cell));
            }
            None => open = Some(Open::new(*cell)),
        }
    }
    out.extend(open.map(Open::close));
    out
}

#[cfg(test)]
mod tests;
