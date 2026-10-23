//! The blocks of one pane at one moment, and every question the views ask of them: the gutter's
//! marks, the sticky header, where the tools go, a block's last and output rows, and the bands'
//! input. Made from a [`Read`] by [`Frame::of`]; nothing here touches the terminal.
//!
//! Rows are screen rows (0 is the oldest kept row) in a [`Read`], and the answers are in the
//! viewport's rows, negative above it, as the views want them.

use crate::block::Outcome;
use crate::vocab::ids::BlockId;

use super::card::{CardBlock, Lead, Mark, Pick, Stage};
use super::place::{Span, ViewRows, command_rows, heads_viewport, tools_row};
use super::read::{BlockRead, End, LiveRead, Read, Spot};

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

/// What the gutter shows for one block whose prompt is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockMark {
    pub id: BlockId,
    /// The prompt's row in the viewport.
    pub row: u32,
    /// The viewport row of the prompt's header line: the row under the prompt's first row when
    /// that row is the blank line temor's prompt leaves above it, else the prompt's row.
    pub header: u32,
    /// How many columns the header line's text takes, up to its last non-blank cell: what is
    /// drawn beside it on the right must start after that.
    pub header_cols: u32,
    pub outcome: Outcome,
    /// How long it ran; `None` while it runs.
    pub took_ms: Option<u64>,
}

/// The blocks of one pane at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    view: ViewRows,
    len: u32,
    first_shown: u32,
    blocks: Vec<BlockRead>,
    live: Option<LiveRead>,
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
            first_shown: read.first_shown,
            blocks: read.blocks,
            live: read.live,
        }
    }

    fn rel(&self, row: u32) -> i64 {
        i64::from(row) - i64::from(self.view.top)
    }

    fn block(&self, id: BlockId) -> Option<&BlockRead> {
        self.blocks.iter().find(|read| read.id == id)
    }

    /// The viewport as it is shown: its top is the first row shown whole.
    fn shown(&self) -> ViewRows {
        ViewRows {
            top: self
                .view
                .top
                .saturating_add(self.first_shown)
                .min(self.view.bottom),
            ..self.view
        }
    }

    /// A block's rows, the prompt (or output) mark through the last row: a running block ends at
    /// the cursor's row, or the viewport's bottom; a finished one on the row before its end mark
    /// when that mark is at a line's start.
    fn span_of(&self, read: &BlockRead) -> Option<Span> {
        let first = read.prompt.or(read.output)?.row;
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
        let first = read.prompt.or(read.output)?.row;
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

    /// The first and last screen rows of a block (a running block runs to the bottom), when the
    /// scrollback still holds it.
    pub fn reach(&self, id: BlockId) -> Option<(u32, u32)> {
        self.blocks
            .iter()
            .find(|read| read.id == id)
            .and_then(Self::reach_of)
    }

    /// The blocks the scrollback still holds, oldest first: those that can be picked.
    pub fn pick_order(&self) -> Vec<BlockId> {
        self.blocks
            .iter()
            .filter(|read| Self::reach_of(read).is_some())
            .map(|read| read.id)
            .collect()
    }

    /// The block under viewport row `row`.
    pub fn block_at_viewport_row(&self, row: u32) -> Option<BlockId> {
        self.block_at_row(self.view.top.saturating_add(row))
            .map(|read| read.id)
    }

    /// The gutter's marks for the blocks whose prompts are in the viewport.
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
                    header_cols: read.header_cols,
                    outcome: read.outcome,
                    took_ms: read.took_ms,
                })
            })
            .collect()
    }

    /// The command that heads the pane while its block's prompt is above the viewport.
    pub fn sticky_header(&self) -> Option<StickyHeader> {
        let view = self.shown();
        let read = self.block_at_row(view.top)?;
        let span = self.span_of(read)?;
        if !heads_viewport(span, &view) {
            return None;
        }
        let command = read.command.lines().next()?.trim();
        match command.is_empty() {
            true => None,
            false => Some(StickyHeader {
                id: read.id,
                command: command.to_owned(),
            }),
        }
    }

    /// Where the tools for `id` go, with `reach` rows to cover: its header row, or the sticky
    /// header when its prompt is scrolled away. `None` for a block with no command, one off the
    /// screen, or when they would cover the cursor.
    pub fn tools_spot(&self, id: BlockId, reach: u32) -> Option<ToolsSpot> {
        let read = self.block(id)?;
        if read.command.is_empty() {
            return None;
        }
        let span = self.span_of(read)?;
        // On the header line, under the blank line temor's prompt leaves above it.
        let lead = match read.prompt {
            Some(_) => read.lead,
            None => Lead::Header,
        };
        let span = match lead {
            Lead::Blank if span.first < span.last => Span {
                first: span.first + 1,
                ..span
            },
            _ => span,
        };
        let row = tools_row(span, &self.shown(), reach.max(1))?;
        Some(ToolsSpot {
            id,
            row: row - self.view.top,
        })
    }

    /// The viewport row of a block's last row: negative above the view, past the last row below.
    pub fn last_row(&self, id: BlockId) -> Option<i64> {
        let span = self.span_of(self.block(id)?)?;
        Some(self.rel(span.last))
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

    /// The cursor's row in the viewport, when it is shown.
    pub fn cursor_row(&self) -> Option<i64> {
        self.view.cursor.map(|row| self.rel(row))
    }

    /// The cards for the bands, oldest first: every block with a prompt, and the prompt being
    /// typed at. `end` is the viewport row a running block's command runs to.
    pub fn cards(&self, end: i64) -> Vec<CardBlock> {
        let top = i64::from(self.view.top);
        let command_of = |input: Option<Spot>, output: Option<Spot>| {
            let input = input?;
            let output = output.map(|at| (i64::from(at.row) - top, at.col));
            let (from, to) = command_rows(i64::from(input.row) - top, output, end);
            Some((from, to, input.col))
        };
        let mut cards: Vec<CardBlock> = self
            .blocks
            .iter()
            .filter_map(|read| {
                let prompt = read.prompt?;
                Some(CardBlock {
                    first: i64::from(prompt.row) - top,
                    lead: read.lead,
                    stage: Stage::Ran(read.outcome),
                    pick: match read.picked {
                        true => Pick::Selected,
                        false => Pick::Unselected,
                    },
                    mark: match read.bookmarked {
                        true => Mark::Bookmarked,
                        false => Mark::Plain,
                    },
                    command: command_of(read.input, read.output),
                })
            })
            .collect();
        if let Some(live) = self.live {
            cards.push(CardBlock {
                first: i64::from(live.prompt.row) - top,
                lead: live.lead,
                stage: Stage::AtPrompt,
                pick: Pick::Unselected,
                mark: Mark::Plain,
                command: command_of(live.input, None),
            });
        }
        cards.sort_by_key(|card| card.first);
        cards
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
