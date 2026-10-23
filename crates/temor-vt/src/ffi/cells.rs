//! Cell text and widths of a screen row, for copying and searching by cell column.

use temor_vt_sys as sys;

use super::convert::check;
use super::terminal::Terminal;
use crate::{CellWidth, GridPoint, PointSpace, VtError};

/// One cell of a screen row: the text it holds (empty when blank or a wide cell's spacer) and
/// its width class.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GridCell {
    pub text: String,
    pub width: CellWidth,
}

impl Terminal {
    /// The cells of screen row `row` (screen space), left to right, one per column. A wide
    /// character is followed by a spacer cell, so cell index and column agree.
    pub fn row_cells(&self, row: u32) -> Result<Vec<GridCell>, VtError> {
        let cols = self.size().cols.0;
        let mut out = Vec::with_capacity(usize::from(cols));
        for x in 0..cols {
            let gref = self.grid_ref(GridPoint::new(PointSpace::Screen, x, row))?;
            let mut cell: sys::Cell = 0;
            // SAFETY: `gref` came from this live terminal; `cell` is a valid out-slot.
            check(
                unsafe { sys::ghostty_grid_ref_cell(&gref, &mut cell) },
                "cell",
            )?;
            let width = width_of(cell);
            // A blank narrow cell is a space: a tab or a cursor move leaves blank cells that copy
            // and search must see. A wide character's spacer stays empty.
            let text = match (has_text(cell), width) {
                (true, _) => graphemes(&gref)?,
                (false, CellWidth::Narrow) => " ".to_owned(),
                (false, _) => String::new(),
            };
            out.push(GridCell { text, width });
        }
        Ok(out)
    }
}

fn width_of(cell: sys::Cell) -> CellWidth {
    let mut wide: sys::CellWide::Type = sys::CellWide::NARROW;
    // SAFETY: `WIDE` writes one `CellWide` enum value, which `wide` is.
    unsafe {
        sys::ghostty_cell_get(
            cell,
            sys::CellData::WIDE,
            (&mut wide as *mut sys::CellWide::Type).cast(),
        );
    }
    match wide {
        sys::CellWide::WIDE => CellWidth::Wide,
        sys::CellWide::SPACER_TAIL => CellWidth::SpacerTail,
        sys::CellWide::SPACER_HEAD => CellWidth::SpacerHead,
        _ => CellWidth::Narrow,
    }
}

pub(super) fn has_text(cell: sys::Cell) -> bool {
    let mut any = false;
    // SAFETY: `HAS_TEXT` writes one `bool`, which `any` is.
    unsafe {
        sys::ghostty_cell_get(
            cell,
            sys::CellData::HAS_TEXT,
            (&mut any as *mut bool).cast(),
        );
    }
    any
}

/// The grapheme cluster of a cell, read with libghostty's two-call sizing protocol.
fn graphemes(gref: &sys::GridRef) -> Result<String, VtError> {
    let mut len = 0usize;
    // SAFETY: a null buffer with zero length asks for the size in `len`.
    let code = unsafe { sys::ghostty_grid_ref_graphemes(gref, std::ptr::null_mut(), 0, &mut len) };
    if code == sys::Result::SUCCESS || len == 0 {
        return Ok(String::new());
    }
    let mut points = vec![0u32; len];
    // SAFETY: `points` has `len` slots, the size libghostty just reported.
    check(
        unsafe { sys::ghostty_grid_ref_graphemes(gref, points.as_mut_ptr(), len, &mut len) },
        "grapheme",
    )?;
    Ok(points[..len.min(points.len())]
        .iter()
        .filter_map(|&c| char::from_u32(c))
        .collect())
}
