//! What the gutter, the bar and the prompt jumps read from the blocks, and what a press on the
//! gutter or a jump does to the view. Nothing here is drawn; the views decide how to show it.

use temor_core::block::Outcome;
use temor_core::vocab::ids::BlockId;
use temor_vt::{PointSpace, SurfacePoint};

use super::super::Session;
use super::super::model::PADDING_LOGICAL;
use super::bands::{Lead, lead_at};
use crate::error::UiError;

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

    /// Picks no block.
    pub fn clear_block_selection(&mut self) {
        self.picks.clear();
    }

    /// The picked block the arrows move (the last one clicked), if any.
    pub fn selected_block(&self) -> Option<BlockId> {
        self.picks.active()
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

    /// A press on the gutter picks the block at that row, as a click on the block does. Returns
    /// whether the press was a gutter press.
    pub fn gutter_press(&mut self, at: SurfacePoint) -> Result<bool, UiError> {
        let Some(id) = self.gutter_block(at) else {
            return Ok(false);
        };
        self.selection = None;
        self.mirror_selection()?;
        self.pick_block(id, temor_core::block::PickClick::Only);
        Ok(true)
    }
}

/// The columns up to and including the last cell that holds more than blank space.
fn used_columns(cells: &[temor_pane::text::RowCell]) -> u32 {
    cells
        .iter()
        .rposition(|cell| !cell.text.trim().is_empty())
        .map_or(0, |last| u32::try_from(last + 1).unwrap_or(u32::MAX))
}
