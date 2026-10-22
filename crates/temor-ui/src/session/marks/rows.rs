//! Where blocks sit in the viewport (PLAN §9 blocks): which block a row or a point belongs to,
//! the block whose command heads the pane while its prompt is scrolled away, where the block
//! toolbar goes, and scrolling a block's prompt to the top. The arithmetic is plain functions on
//! rows; the `Session` methods read the rows from the terminal and call them.

use temor_core::vocab::ids::BlockId;
use temor_vt::{PointSpace, RowCount, SurfacePoint, ViewportScroll};

use super::super::Session;
use super::bands::{Lead, command_rows, lead_at};

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
    /// The viewport in screen rows; `None` on a full-screen program's screen (the shell's blocks
    /// are not shown there) or before the first layout.
    fn view_rows(&self) -> Option<ViewRows> {
        let (viewport, alternate) = self
            .pane
            .with_term(|term| (term.viewport().ok(), term.on_alternate_screen()));
        if alternate {
            return None;
        }
        let (top, len) = viewport?;
        let cursor = self
            .painter
            .cursor_cell()
            .map(|cell| top.saturating_add(u32::from(cell.row.0)));
        Some(ViewRows {
            top,
            bottom: top.saturating_add(len.saturating_sub(1)),
            cursor,
        })
    }

    /// The viewport as it is shown: its top is the first row shown whole, below the rows the
    /// room under a block pushed out of the pane.
    fn shown_rows(&self, view: ViewRows) -> ViewRows {
        let first = self
            .layout
            .as_ref()
            .map_or(0, |layout| u32::from(layout.visible_rows().start));
        ViewRows {
            top: view.top.saturating_add(first).min(view.bottom),
            ..view
        }
    }

    /// A block's rows. A running block ends at the cursor's row, or at the viewport's bottom
    /// when the cursor is not shown; a finished one ends on the row before its end mark when
    /// that mark is at a line's start.
    fn span_of(&self, id: BlockId, view: &ViewRows) -> Option<Span> {
        let anchors = self.blocks.anchors.get(&id)?;
        let first = self.screen_row(anchors.prompt.or(anchors.output)?)?;
        let last = match anchors.end {
            Some(end) => {
                let at = self
                    .pane
                    .with_term(|term| term.anchor_point(end, PointSpace::Screen))?;
                match at.col.0 {
                    0 if at.row > first => at.row - 1,
                    _ => at.row,
                }
            }
            None => view.cursor.unwrap_or(view.bottom),
        };
        Some(Span {
            first,
            last: last.max(first),
        })
    }

    /// The block under a point in the pane (device pixels), anywhere in the pane. `None` on a
    /// full-screen program's screen.
    pub fn block_at_point(&self, at: SurfacePoint) -> Option<BlockId> {
        let placed = self.placed()?;
        let view = self.view_rows()?;
        let hit = placed.cell_at(at);
        self.block_at_row(view.top.saturating_add(u32::from(hit.pos.row.0)))
    }

    /// The command to show at the top of the pane: the block holding the top row, when its
    /// prompt is above the viewport and enough of it shows. `None` on a full-screen program's
    /// screen and for a block with no command.
    pub fn sticky_header(&self) -> Option<StickyHeader> {
        let view = self.shown_rows(self.view_rows()?);
        let id = self.block_at_row(view.top)?;
        let span = self.span_of(id, &view)?;
        if !heads_viewport(span, &view) {
            return None;
        }
        let command = self.block_command(id)?.lines().next()?.trim();
        match command.is_empty() {
            true => None,
            false => Some(StickyHeader {
                id,
                command: command.to_owned(),
            }),
        }
    }

    /// Where the tools for `id` go: its header row, or the sticky header when its prompt is
    /// scrolled away. `None` for a block with no command, one off the screen, or when they would
    /// cover the cursor.
    pub fn tools_spot(&self, id: BlockId) -> Option<ToolsSpot> {
        let view = self.view_rows()?;
        if !self.block_command(id).is_some_and(|c| !c.is_empty()) {
            return None;
        }
        let span = self.span_of(id, &view)?;
        // On the header line, under the blank line temor's prompt leaves above it.
        let anchors = self.blocks.anchors.get(&id)?;
        let lead = match anchors.prompt {
            Some(prompt) => self
                .pane
                .with_term(|term| lead_at(term, prompt, anchors.input)),
            None => Lead::Header,
        };
        let span = match lead {
            Lead::Blank if span.first < span.last => Span {
                first: span.first + 1,
                ..span
            },
            _ => span,
        };
        let reach = self.gutter_geometry().map_or(1, |gutter| {
            (self.tools_height.get() / gutter.cell_h.max(1.0)).ceil() as u32
        });
        let row = tools_row(span, &self.shown_rows(view), reach.max(1))?;
        Some(ToolsSpot {
            id,
            row: row - view.top,
        })
    }

    /// The viewport row of a block's last row: negative above the view, past the last row below
    /// it. Where a picture under the block starts is the row after. `None` on a full-screen
    /// program's screen and for a block with no rows.
    pub fn block_last_row(&self, id: BlockId) -> Option<i64> {
        let view = self.view_rows()?;
        let span = self.span_of(id, &view)?;
        Some(i64::from(span.last) - i64::from(view.top))
    }

    /// The viewport rows of a block's output, first to last: negative above the view, past the
    /// last row below it. `None` on a full-screen program's screen and for a block with no row
    /// under its command.
    pub fn block_output_rows(&self, id: BlockId) -> Option<(i64, i64)> {
        let view = self.view_rows()?;
        let span = self.span_of(id, &view)?;
        let anchors = self.blocks.anchors.get(&id)?;
        let input = i64::from(self.screen_row(anchors.input?)?);
        let output = anchors
            .output
            .and_then(|mark| {
                self.pane
                    .with_term(|term| term.anchor_point(mark, PointSpace::Screen))
            })
            .map(|at| (i64::from(at.row), at.col.0));
        let (_, command_end) = command_rows(input, output, input);
        let (first, last) = (command_end + 1, i64::from(span.last));
        let top = i64::from(view.top);
        (first <= last).then_some((first - top, last - top))
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
