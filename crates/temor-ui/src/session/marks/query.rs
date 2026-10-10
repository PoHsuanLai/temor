//! What the gutter, the bar and the prompt jumps read from the blocks, and what a press on the
//! gutter or a jump does to the view. Nothing here is drawn; the views decide how to show it.

use temor_core::block::{Outcome, format_elapsed, next_prompt, previous_prompt};
use temor_core::vocab::ids::{BlockId, GridRow};
use temor_pane::text::{CellPos, Granularity, Selection, Shape};
use temor_vt::{PointSpace, RowCount, SurfacePoint, ViewportScroll};

use super::super::Session;
use super::super::model::PADDING_LOGICAL;
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
    pub outcome: Outcome,
    /// How long it ran; `None` while it runs.
    pub took_ms: Option<u64>,
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
                    let prompt = self.blocks.anchors.get(&block.id)?.prompt?;
                    let at = term.anchor_point(prompt, PointSpace::Viewport)?;
                    Some(BlockMark {
                        id: block.id,
                        row: at.row,
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

    /// How a block ended and how long it ran, for its hover and its notices.
    pub fn block_summary(&self, id: BlockId) -> Option<String> {
        let block = self.blocks.store.get(id)?;
        let took = block.elapsed_ms().map(format_elapsed);
        Some(match (block.outcome(), took) {
            (Outcome::Running, _) => "Still running".to_owned(),
            (Outcome::Succeeded, Some(took)) => format!("Succeeded in {took}"),
            (Outcome::Failed(code), Some(took)) => {
                format!("Failed with status {} after {took}", code.0)
            }
            (Outcome::Stopped(_), Some(took)) => format!("Stopped after {took}"),
            (Outcome::Unreported, Some(took)) => format!("Ended after {took}"),
            (_, None) => "Ended".to_owned(),
        })
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

    /// The pane's logical padding and cell height, for placing things beside grid rows.
    pub fn gutter_geometry(&self) -> Option<(f32, f32)> {
        let fit = self.fit?;
        let factor = self.scale.factor();
        Some((PADDING_LOGICAL, fit.cell.height.0 as f32 / factor))
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
