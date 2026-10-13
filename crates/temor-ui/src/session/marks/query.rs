//! What the gutter, the bar and the prompt jumps read from the blocks, and what a press on the
//! gutter or a jump does to the view. Nothing here is drawn; the views decide how to show it.

use temor_core::block::{Outcome, next_prompt, previous_prompt};
use temor_core::vocab::ids::{BlockId, GridRow};
use temor_pane::text::{CellPos, Granularity, Selection, Shape};
use temor_vt::{PointSpace, RowCount, SurfacePoint, ViewportScroll};

use super::super::Session;
use super::super::model::PADDING_LOGICAL;
use super::bands::{Lead, lead_at};
use crate::error::UiError;

/// Which way a jump between prompts goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptStep {
    /// Towards older output (⌘↑).
    Previous,
    /// Towards the live prompt (⌘↓).
    Next,
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

/// Where the grid's rows sit in the pane, in logical pixels: the inset on the left and right,
/// the inset above the first row, and a row's height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gutter {
    pub side: f32,
    pub top: f32,
    pub cell_h: f32,
}

impl Gutter {
    /// The top of viewport row `row`.
    pub fn row_top(&self, row: u32) -> f32 {
        self.top + row as f32 * self.cell_h
    }
}

impl Session {
    /// How many primary prompts the shell has marked so far.
    pub fn prompts_seen(&self) -> usize {
        self.blocks.prompts
    }

    /// Every block the session holds, oldest first, with how each stands.
    pub fn block_outcomes(&self) -> Vec<(BlockId, Outcome)> {
        self.blocks
            .store
            .iter()
            .map(|block| (block.id, block.outcome()))
            .collect()
    }

    /// Whether a block's command has finished (its output is complete).
    pub fn block_finished(&self, id: BlockId) -> bool {
        self.blocks
            .store
            .get(id)
            .is_some_and(|block| block.outcome() != Outcome::Running)
    }

    /// The gutter's marks for the blocks whose prompts are on screen. None while a full-screen
    /// program holds the screen: the shell's blocks are not shown there.
    pub fn block_marks(&self) -> Vec<BlockMark> {
        self.pane.with_term(|term| {
            if term.on_alternate_screen() {
                return Vec::new();
            }
            self.blocks
                .store
                .iter()
                .filter_map(|block| {
                    let anchors = self.blocks.anchors.get(&block.id)?;
                    let at = term.anchor_point(anchors.prompt?, PointSpace::Viewport)?;
                    let below = match lead_at(term, anchors.prompt?, anchors.input) {
                        Lead::Blank => 1,
                        Lead::Header => 0,
                    };
                    let header_cols = term
                        .anchor_point(anchors.prompt?, PointSpace::Screen)
                        .and_then(|first| term.row_text(first.row + below).ok())
                        .map_or(0, |text| used_columns(&text.cells));
                    Some(BlockMark {
                        id: block.id,
                        row: at.row,
                        header: at.row + below,
                        header_cols,
                        outcome: block.outcome(),
                        took_ms: block.elapsed_ms(),
                    })
                })
                .collect()
        })
    }

    /// Clears the block the gutter selected.
    pub fn clear_block_selection(&mut self) {
        self.selected_block = None;
    }

    /// The block the gutter has selected (PLAN §9: a click on its gutter).
    pub fn selected_block(&self) -> Option<BlockId> {
        self.selected_block
    }

    /// The command a block ran, as typed.
    pub fn block_command(&self, id: BlockId) -> Option<&str> {
        self.blocks.store.get(id).map(|b| b.command.0.as_str())
    }

    /// A block's output as text, once the command has finished.
    pub fn block_output(&self, id: BlockId) -> Option<String> {
        let anchors = self.blocks.anchors.get(&id)?;
        self.text_between(anchors.output?, anchors.end?)
    }

    /// The output of the block that holds screen row `row`, once its command has finished.
    pub fn output_at_row(&self, row: u32) -> Option<String> {
        self.block_at_row(row).and_then(|id| self.block_output(id))
    }

    /// How a block ended and how long it ran, as its card header says it (`exit 1 · 4 ms`);
    /// `None` while it runs.
    pub fn block_meta(&self, id: BlockId) -> Option<String> {
        let block = self.blocks.store.get(id)?;
        crate::view::block_meta::meta_text(block.outcome(), block.elapsed_ms())
    }

    /// Moves the view to the previous or next prompt (⌘↑ and ⌘↓).
    pub(in crate::session) fn jump_prompt(&mut self, step: PromptStep) -> Result<(), UiError> {
        let (prompts, top) = self.pane.with_term(|term| {
            let prompts: Vec<(BlockId, GridRow)> = self
                .blocks
                .store
                .iter()
                .filter_map(|block| {
                    let prompt = self.blocks.anchors.get(&block.id)?.prompt?;
                    let at = term.anchor_point(prompt, PointSpace::Screen)?;
                    Some((block.id, GridRow(u64::from(at.row))))
                })
                .collect();
            let top = term.viewport().map_or(0, |(offset, _)| u64::from(offset));
            (prompts, GridRow(top))
        });
        let target = match step {
            PromptStep::Previous => previous_prompt(&prompts, top),
            PromptStep::Next => next_prompt(&prompts, top),
        };
        if let Some((_, row)) = target.and_then(|id| prompts.iter().find(|(b, _)| *b == id)) {
            self.pane
                .with_term(|term| term.scroll(ViewportScroll::Row(RowCount(row.0))));
        }
        Ok(())
    }

    /// The block whose gutter is under `at` (device pixels in the pane), if the point is in the
    /// left margin, on a row that belongs to a block. Never on a full-screen program's screen.
    pub fn gutter_block(&self, at: SurfacePoint) -> Option<BlockId> {
        let fit = self.fit?;
        let margin = i64::from(fit.padding.left.0);
        if at.x < 0 || i64::from(at.x) >= margin {
            return None;
        }
        if self.pane.with_term(|term| term.on_alternate_screen()) {
            return None;
        }
        let hit = fit.cell_at(at);
        let top = self.viewport_top().ok()?;
        let row = top.saturating_add(u32::from(hit.pos.row.0));
        self.block_at_row(row)
    }

    /// Where the grid's rows sit in the pane, for placing things beside them.
    pub fn gutter_geometry(&self) -> Option<Gutter> {
        let fit = self.fit?;
        let factor = self.scale.factor();
        Some(Gutter {
            side: PADDING_LOGICAL,
            top: fit.padding.top.0 as f32 / factor,
            cell_h: fit.cell.height.0 as f32 / factor,
        })
    }

    /// A press on the gutter selects the block at that row, its output included. Returns whether
    /// the press was a gutter press.
    pub fn gutter_press(&mut self, at: SurfacePoint) -> Result<bool, UiError> {
        let Some(id) = self.gutter_block(at) else {
            return Ok(false);
        };
        self.select_block(id)?;
        self.selected_block = Some(id);
        Ok(true)
    }

    /// Selects a finished block's output, the way a drag over it would.
    fn select_block(&mut self, id: BlockId) -> Result<(), UiError> {
        let Some(anchors) = self.blocks.anchors.get(&id).copied() else {
            return Ok(());
        };
        let (Some(output), Some(end)) = (anchors.output, anchors.end) else {
            return Ok(());
        };
        let (Some(first), Some(stop)) = (self.screen_row(output), self.screen_row(end)) else {
            return Ok(());
        };
        let end_at = self
            .pane
            .with_term(|term| term.anchor_point(end, PointSpace::Screen));
        // The end mark sits at the start of the line after the output, so stop one line short.
        let last = match end_at {
            Some(at) if at.col.0 == 0 && stop > first => stop - 1,
            _ => stop,
        };
        let line = self.line(first)?;
        let mut selection = Selection::begin(
            CellPos::new(first, 0),
            &line,
            Granularity::Cells,
            Shape::Linear,
        );
        let last_line = self.line(last)?;
        let width = u16::try_from(last_line.cells.len().saturating_sub(1)).unwrap_or(u16::MAX);
        selection.extend(CellPos::new(last, width), &last_line);
        self.selection = Some(selection);
        self.mirror_selection()
    }
}

/// The columns up to and including the last cell that holds more than blank space.
fn used_columns(cells: &[temor_pane::text::RowCell]) -> u32 {
    cells
        .iter()
        .rposition(|cell| !cell.text.trim().is_empty())
        .map_or(0, |last| u32::try_from(last + 1).unwrap_or(u32::MAX))
}
