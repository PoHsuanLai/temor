//! Where blocks sit in the viewport (PLAN §9 blocks): which block a row or a point belongs to,
//! the block whose command heads the pane while its prompt is scrolled away, where the block
//! toolbar goes, and scrolling a block's prompt to the top. The arithmetic is plain functions on
//! rows; the `Session` methods read the rows from the terminal and call them.

use temor_core::vocab::ids::BlockId;
use temor_vt::{RowCount, SurfacePoint, ViewportScroll};

use super::super::Session;

#[cfg(test)]
mod tests;

/// How many rows of a block must show for its command to head the pane. Fewer, and the header
/// would cover most of what is left of the block.
pub const HEADER_MIN_ROWS: u32 = 2;

/// The tools' height in logical pixels before the window has measured it (`ToolsHeight`), to
/// count the grid rows they cover: the dense control size.
pub const TOOLS_HEIGHT_LOGICAL: f32 = 20.0;

/// The command that heads the pane while its block's prompt is above the viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyHeader {
    pub id: BlockId,
    /// The command's first line.
    pub command: String,
}

/// Where the block tools go: the block they act on and the viewport row they are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolsSpot {
    pub id: BlockId,
    /// A viewport row (0 is the top row).
    pub row: u32,
}

/// A block's rows in the screen (0 is the oldest kept row), prompt through last output row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) first: u32,
    pub(super) last: u32,
}

/// The viewport, in screen rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ViewRows {
    /// The first row shown.
    pub(super) top: u32,
    /// The last row shown.
    pub(super) bottom: u32,
    /// The cursor's row, when it is shown.
    pub(super) cursor: Option<u32>,
}

/// Whether a block heads the viewport: its prompt is above the top and at least
/// [`HEADER_MIN_ROWS`] of its rows show.
pub(super) fn heads_viewport(span: Span, view: &ViewRows) -> bool {
    let shown = span.last.min(view.bottom).saturating_add(1);
    span.first < view.top && shown.saturating_sub(view.top) >= HEADER_MIN_ROWS
}

/// The screen row the tools go on: the block's header row (its prompt's row) when that is in
/// view, else the top row, which the sticky header covers, when the block heads the viewport.
/// `None` when the block has neither, or when the tools, `reach` rows tall, would cover the
/// cursor's row.
pub(super) fn tools_row(span: Span, view: &ViewRows, reach: u32) -> Option<u32> {
    let row = match span.first >= view.top {
        true => span.first,
        false if heads_viewport(span, view) => view.top,
        false => return None,
    };
    if row > span.last.min(view.bottom) {
        return None;
    }
    let covers_cursor = view
        .cursor
        .is_some_and(|cursor| cursor >= row && cursor < row.saturating_add(reach));
    match covers_cursor {
        true => None,
        false => Some(row),
    }
}

impl Session {
    /// The block under a point in the pane (device pixels), anywhere in the pane. `None` on a
    /// full-screen program's screen.
    pub fn block_at_point(&self, at: SurfacePoint) -> Option<BlockId> {
        let placed = self.placed()?;
        let hit = placed.cell_at(at);
        self.frame()?
            .block_at_viewport_row(u32::from(hit.pos.row.0))
    }

    /// How many grid rows the block tools cover.
    pub fn tools_reach(&self) -> u32 {
        self.gutter_geometry().map_or(1, |gutter| {
            (self.tools_height.get() / gutter.cell_h.max(1.0)).ceil() as u32
        })
    }

    /// The viewport row of a block's last row: negative above the view, past the last row below
    /// it. Where a picture under the block starts is the row after. `None` on a full-screen
    /// program's screen and for a block with no rows.
    pub fn block_last_row(&self, id: BlockId) -> Option<i64> {
        self.frame()?.last_row(id)
    }

    /// The viewport rows of a block's output, first to last: negative above the view, past the
    /// last row below it. `None` on a full-screen program's screen and for a block with no row
    /// under its command.
    pub fn block_output_rows(&self, id: BlockId) -> Option<(i64, i64)> {
        self.frame()?.output_rows(id)
    }

    /// Device pixels to a logical pixel on this pane's screen.
    pub fn scale_factor(&self) -> f32 {
        self.scale.factor()
    }

    /// Scrolls so a block's prompt is the top row.
    pub fn scroll_block_to_top(&mut self, id: BlockId) {
        let Some((first, _)) = self.block_rows(id) else {
            return;
        };
        self.pane
            .with_term(|term| term.scroll(ViewportScroll::Row(RowCount(u64::from(first)))));
    }
}
