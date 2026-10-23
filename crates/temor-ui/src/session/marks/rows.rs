//! Where blocks sit in the viewport (PLAN §9 blocks): which block a row or a point belongs to,
//! the block whose command heads the pane while its prompt is scrolled away, where the block
//! toolbar goes, and scrolling a block's prompt to the top. The arithmetic is plain functions on
//! rows; the `Session` methods read the rows from the terminal and call them.

use temor_core::vocab::ids::BlockId;
use temor_vt::{RowCount, SurfacePoint, ViewportScroll};

use super::super::Session;

/// The tools' height in logical pixels before the window has measured it (`ToolsHeight`), to
/// count the grid rows they cover: the dense control size.
pub const TOOLS_HEIGHT_LOGICAL: f32 = 20.0;

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

    /// Device pixels to a logical pixel on this pane's screen.
    pub fn scale_factor(&self) -> f32 {
        self.scale.factor()
    }

    /// Scrolls so a block's prompt is the top row.
    pub fn scroll_block_to_top(&mut self, id: BlockId) {
        let Some((first, _)) = self.frame().and_then(|frame| frame.reach(id)) else {
            return;
        };
        self.pane
            .with_term(|term| term.scroll(ViewportScroll::Row(RowCount(u64::from(first)))));
    }
}
