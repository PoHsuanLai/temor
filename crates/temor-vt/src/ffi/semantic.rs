//! Where shell integration's kinds of text start on the screen, read from the cells: a screen
//! restored from a snapshot keeps each cell's kind (prompt, typed command, output), but its OSC
//! 133 marks were never seen as events, so the window finds its blocks again from these.

use temor_vt_sys as sys;

use super::cells::has_text;
use super::convert::check;
use super::row::semantic;
use super::terminal::Terminal;
use crate::{GridPoint, PointSpace, SemanticMark, VtError};

impl Terminal {
    /// Every point, top of the scrollback to the bottom of the screen, where the kind of the text
    /// changes: the first cell with text of a new kind. Only cells with text count, so the blank
    /// cells between a prompt and its typed command do not split them.
    pub fn semantic_starts(&self) -> Result<Vec<(GridPoint, SemanticMark)>, VtError> {
        let rows = self.scrollbar()?.total.0;
        let cols = self.size().cols.0;
        let mut out = Vec::new();
        let mut last: Option<SemanticMark> = None;
        for row in 0..u32::try_from(rows).unwrap_or(u32::MAX) {
            for col in 0..cols {
                let at = GridPoint::new(PointSpace::Screen, col, row);
                let gref = self.grid_ref(at)?;
                let mut cell: sys::Cell = 0;
                // SAFETY: `gref` came from this live terminal; `cell` is a valid out-slot.
                check(
                    unsafe { sys::ghostty_grid_ref_cell(&gref, &mut cell) },
                    "cell",
                )?;
                if !has_text(cell) {
                    continue;
                }
                let kind = semantic(cell);
                if last != Some(kind) {
                    out.push((at, kind));
                    last = Some(kind);
                }
            }
        }
        Ok(out)
    }
}
