//! The blocks of one pane at one moment, and the questions the list asks of them: which block
//! holds a row, the order blocks are picked in, the prompts in view and a block's output rows.
//! Made from a [`Read`] by [`Frame::of`]; nothing here touches the terminal.
//!
//! Rows are screen rows (0 is the oldest kept row) in a [`Read`], and the answers are in the
//! viewport's rows, negative above it, as the views want them.

use crate::vocab::ids::BlockId;

use super::place::{Span, ViewRows, command_rows};
use super::read::{BlockRead, End, Lead, Read};

/// A block whose prompt is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockMark {
    pub id: BlockId,
    /// The prompt's row in the viewport.
    pub row: u32,
    /// The viewport row of the prompt's header line: the row under the prompt's first row when
    /// that row is the blank line temor's prompt leaves above it, else the prompt's row.
    pub header: u32,
}

/// The blocks of one pane at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    view: ViewRows,
    len: u32,
    blocks: Vec<BlockRead>,
}

impl Frame {
    pub fn of(read: Read) -> Self {
        let bottom = read.top.saturating_add(read.len.saturating_sub(1));
        Self {
            view: ViewRows {
                top: read.top,
                bottom,
                cursor: read.cursor,
            },
            len: read.len,
            blocks: read.blocks,
        }
    }

    fn rel(&self, row: u32) -> i64 {
        i64::from(row) - i64::from(self.view.top)
    }

    fn block(&self, id: BlockId) -> Option<&BlockRead> {
        self.blocks.iter().find(|read| read.id == id)
    }

    /// A block's first screen row: its prompt's, or its output's when it never had a prompt.
    /// `None` once the scrollback took the prompt.
    fn first_row(read: &BlockRead) -> Option<u32> {
        match read.prompt_lost {
            true => None,
            false => Some(read.prompt.or(read.output)?.row),
        }
    }

    /// A block's rows, the prompt (or output) mark through the last row: a running block ends at
    /// the cursor's row, or the viewport's bottom; a finished one on the row before its end mark
    /// when that mark is at a line's start.
    fn span_of(&self, read: &BlockRead) -> Option<Span> {
        let first = Self::first_row(read)?;
        let last = match read.end {
            End::At(end) => match end.col {
                0 if end.row > first => end.row - 1,
                _ => end.row,
            },
            End::Running => self.view.cursor.unwrap_or(self.view.bottom),
            End::Lost => return None,
        };
        Some(Span {
            first,
            last: last.max(first),
        })
    }

    /// The first and last screen rows of a block, a running block running to the end of the
    /// screen: what a row belongs to.
    fn reach_of(read: &BlockRead) -> Option<(u32, u32)> {
        let first = Self::first_row(read)?;
        let last = match read.end {
            End::At(end) => end.row,
            End::Running => u32::MAX,
            End::Lost => return None,
        };
        Some((first, last))
    }

    /// The newest block whose rows hold screen row `row`.
    fn block_at_row(&self, row: u32) -> Option<&BlockRead> {
        self.blocks.iter().rev().find(|read| {
            Self::reach_of(read).is_some_and(|(first, last)| (first..=last).contains(&row))
        })
    }

    /// The newest block whose rows hold screen row `row`.
    pub fn block_at_screen_row(&self, row: u32) -> Option<BlockId> {
        self.block_at_row(row).map(|read| read.id)
    }

    /// The blocks the scrollback still holds, oldest first: those that can be picked.
    pub fn pick_order(&self) -> Vec<BlockId> {
        self.blocks
            .iter()
            .filter(|read| Self::reach_of(read).is_some())
            .map(|read| read.id)
            .collect()
    }

    /// The blocks whose prompts are in the viewport.
    pub fn marks(&self) -> Vec<BlockMark> {
        self.blocks
            .iter()
            .filter_map(|read| {
                let prompt = read.prompt?;
                let row = u32::try_from(self.rel(prompt.row)).ok()?;
                (row < self.len).then_some(())?;
                let below = match read.lead {
                    Lead::Blank => 1,
                    Lead::Header => 0,
                };
                Some(BlockMark {
                    id: read.id,
                    row,
                    header: row + below,
                })
            })
            .collect()
    }

    /// The viewport rows of a block's output, first to last; `None` for a block with no row
    /// under its command.
    pub fn output_rows(&self, id: BlockId) -> Option<(i64, i64)> {
        let read = self.block(id)?;
        let span = self.span_of(read)?;
        let input = i64::from(read.input?.row);
        let output = read.output.map(|at| (i64::from(at.row), at.col));
        let (_, command_end) = command_rows(input, output, input);
        let (first, last) = (command_end + 1, i64::from(span.last));
        let top = i64::from(self.view.top);
        (first <= last).then_some((first - top, last - top))
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
