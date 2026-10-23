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

impl Terminal {
    /// Takes the prompt left open at the bottom off the screen: the rows from its prompt mark
    /// down are erased and the cursor goes there, as the shell does before it draws a prompt
    /// again. For a screen restored for a new shell, which draws its own prompt. Nothing happens
    /// on the alternate screen, when the last command has output after its prompt (it was
    /// running), or when the prompt is above the screen.
    pub fn take_open_prompt_off(&mut self) {
        if self.on_alternate_screen() {
            return;
        }
        let Ok(starts) = self.semantic_starts() else {
            return;
        };
        let open = starts
            .iter()
            .rposition(|(_, kind)| *kind == SemanticMark::Prompt)
            .filter(|&at| starts[at..].iter().all(|(_, k)| *k != SemanticMark::Output));
        let Some(at) = open else {
            return;
        };
        // The prompt mark can sit on a row above the prompt's text (temor's own prompt leaves a
        // blank line there): those rows go with it.
        let mut row = starts[at].0.row;
        while row > 0 && self.row_marked_prompt(row - 1) && !self.row_has_text(row - 1) {
            row -= 1;
        }
        let Ok(bar) = self.scrollbar() else {
            return;
        };
        let rows = u64::from(self.size().rows.0);
        let top = bar.total.0.saturating_sub(rows);
        let Some(active) = u64::from(row).checked_sub(top) else {
            return;
        };
        self.write(format!("\x1b[{};1H\x1b[J", active + 1).as_bytes());
    }

    fn row_marked_prompt(&self, row: u32) -> bool {
        let Ok(gref) = self.grid_ref(GridPoint::new(PointSpace::Screen, 0, row)) else {
            return false;
        };
        let mut raw: sys::Row = 0;
        // SAFETY: `gref` came from this live terminal; `raw` is a valid out-slot.
        let code = unsafe { sys::ghostty_grid_ref_row(&gref, &mut raw) };
        if code != sys::Result::SUCCESS {
            return false;
        }
        let mut mark: sys::RowSemanticPrompt::Type = sys::RowSemanticPrompt::NONE;
        // SAFETY: SEMANTIC_PROMPT writes one `RowSemanticPrompt` value, which `mark` is.
        unsafe { sys::ghostty_row_get(raw, sys::RowData::SEMANTIC_PROMPT, (&raw mut mark).cast()) };
        mark != sys::RowSemanticPrompt::NONE
    }

    fn row_has_text(&self, row: u32) -> bool {
        (0..self.size().cols.0).any(|col| {
            let Ok(gref) = self.grid_ref(GridPoint::new(PointSpace::Screen, col, row)) else {
                return false;
            };
            let mut cell: sys::Cell = 0;
            // SAFETY: `gref` came from this live terminal; `cell` is a valid out-slot.
            let code = unsafe { sys::ghostty_grid_ref_cell(&gref, &mut cell) };
            code == sys::Result::SUCCESS && has_text(cell)
        })
    }
}
