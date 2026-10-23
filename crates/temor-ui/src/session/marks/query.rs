//! What the gutter, the bar and the prompt jumps read from the blocks, and what a press on the
//! gutter or a jump does to the view. Nothing here is drawn; the views decide how to show it.

use temor_core::block::Outcome;
use temor_core::facts::Facts;
use temor_core::session::AnnotateIn;
use temor_core::vocab::ids::BlockId;
use temor_render::RowLayout;
use temor_vt::{Row, SurfacePoint};

use super::super::Session;
use super::super::model::PADDING_LOGICAL;
use crate::error::UiError;
use crate::plugins::facts::PluginFact;
use temor_core::session::{HeaderFacts, MarkIn};

/// Where the grid's rows sit in the pane, in logical pixels: the inset on the left and right,
/// the inset above the first row as drawn, a row's height, and where the rows are shown around
/// the room under blocks with pictures.
#[derive(Debug, Clone, PartialEq)]
pub struct Gutter {
    pub side: f32,
    pub top: f32,
    pub cell_h: f32,
    /// Device pixels to a logical pixel.
    pub factor: f32,
    pub layout: RowLayout,
}

impl Gutter {
    /// The top of viewport row `row` as it is shown.
    pub fn row_top(&self, row: u32) -> f32 {
        let shift = self
            .layout
            .row_shift(Row(u16::try_from(row).unwrap_or(u16::MAX)));
        self.top + row as f32 * self.cell_h + shift as f32 / self.factor.max(f32::EPSILON)
    }

    /// The first viewport row shown whole (rows above it are cut by the room below them).
    pub fn first_shown(&self) -> u32 {
        u32::from(self.layout.visible_rows().start)
    }

    /// Where the room under viewport row `after` begins, when a picture has room there.
    pub fn insert_top(&self, after: i64) -> Option<f32> {
        let row = u16::try_from(after).ok()?;
        let top = self.layout.insert_top(Row(row))?;
        Some(top as f32 / self.factor.max(f32::EPSILON))
    }
}

impl Session {
    /// Keeps what the chips over the prompt being typed at say, for the block it becomes: an
    /// older block's header carries the chips its prompt had. Nothing while no prompt is typed at.
    pub fn keep_header_facts(&mut self, folder: &str, facts: &Facts, plugin: &[PluginFact]) {
        self.mark(MarkIn::Chips(HeaderFacts {
            folder: folder.to_owned(),
            facts: facts.clone(),
            plugin: plugin.to_vec(),
        }));
    }

    /// What the chips over a block's header line said when its prompt was typed at.
    pub fn header_facts(&self, id: BlockId) -> Option<&HeaderFacts> {
        self.blocks.marks.header_facts(id)
    }

    /// How many primary prompts the shell has marked so far.
    pub fn prompts_seen(&self) -> usize {
        self.blocks.marks.prompts()
    }

    /// Every block the session holds, oldest first, with how each stands.
    pub fn block_outcomes(&self) -> Vec<(BlockId, Outcome)> {
        self.blocks
            .store()
            .iter()
            .map(|block| (block.id, block.outcome()))
            .collect()
    }

    /// Whether a block's command has finished (its output is complete).
    pub fn block_finished(&self, id: BlockId) -> bool {
        self.blocks
            .store()
            .get(id)
            .is_some_and(|block| block.outcome() != Outcome::Running)
    }

    /// Picks no block.
    pub fn clear_block_selection(&mut self) {
        self.annotate(AnnotateIn::ClearPicks);
    }

    /// The picked block the arrows move (the last one clicked), if any.
    pub fn selected_block(&self) -> Option<BlockId> {
        self.ann.active_pick()
    }

    /// The command a block ran, as typed.
    pub fn block_command(&self, id: BlockId) -> Option<&str> {
        self.blocks.store().get(id).map(|b| b.command.0.as_str())
    }

    /// A block's output as text, once the command has finished.
    pub fn block_output(&self, id: BlockId) -> Option<String> {
        let anchors = self.blocks.anchors_of(id)?;
        self.text_between(anchors.output?, anchors.end?)
    }

    /// The output of the block that holds screen row `row`, once its command has finished.
    pub fn output_at_row(&self, row: u32) -> Option<String> {
        self.block_at_row(row).and_then(|id| self.block_output(id))
    }

    /// How a block ended and how long it ran, as its card header says it (`exit 1 · 4 ms`);
    /// `None` while it runs.
    pub fn block_meta(&self, id: BlockId) -> Option<String> {
        let block = self.blocks.store().get(id)?;
        crate::view::block_meta::meta_text(block.outcome(), block.elapsed_ms())
    }

    /// The block whose gutter is under `at` (device pixels in the pane), if the point is in the
    /// left margin, on a row that belongs to a block. Never on a full-screen program's screen.
    pub fn gutter_block(&self, at: SurfacePoint) -> Option<BlockId> {
        let placed = self.placed()?;
        let margin = i64::from(placed.fit.padding.left.0);
        if at.x < 0 || i64::from(at.x) >= margin {
            return None;
        }
        if self.pane.with_term(|term| term.on_alternate_screen()) {
            return None;
        }
        let hit = placed.cell_at(at);
        let top = self.viewport_top().ok()?;
        let row = top.saturating_add(u32::from(hit.pos.row.0));
        self.block_at_row(row)
    }

    /// Where the grid's rows sit in the pane, for placing things beside them.
    pub fn gutter_geometry(&self) -> Option<Gutter> {
        let placed = self.placed()?;
        let (fit, factor) = (placed.fit, self.scale.factor());
        Some(Gutter {
            side: PADDING_LOGICAL,
            top: fit.padding.top.0 as f32 / factor,
            cell_h: fit.cell.height.0 as f32 / factor,
            factor,
            layout: placed.layout,
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
pub(super) fn used_columns(cells: &[temor_pane::text::RowCell]) -> u32 {
    cells
        .iter()
        .rposition(|cell| !cell.text.trim().is_empty())
        .map_or(0, |last| u32::try_from(last + 1).unwrap_or(u32::MAX))
}
