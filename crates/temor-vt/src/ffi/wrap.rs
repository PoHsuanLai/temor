//! Soft wraps: whether a screen row continues on the next one. Copying joins such rows without
//! a line break, as libghostty's own formatter does.

use std::ffi::c_void;

use temor_vt_sys as sys;

use super::convert::check;
use super::terminal::Terminal;
use crate::{GridPoint, PointSpace, VtError, Wrap};

impl Terminal {
    /// Whether screen row `row` (in the screen space, scrollback included) is soft-wrapped into
    /// the row after it.
    pub fn row_wraps(&self, row: u32) -> Result<Wrap, VtError> {
        let gref = self.grid_ref(GridPoint::new(PointSpace::Screen, 0, row))?;
        let mut row_handle: sys::Row = 0;
        // SAFETY: `gref` came from this live terminal and `row_handle` is a valid out-pointer.
        check(
            unsafe { sys::ghostty_grid_ref_row(&gref, &mut row_handle) },
            "row",
        )?;
        let mut wraps = false;
        // SAFETY: `WRAP` writes one `bool`, and `wraps` is a valid `bool` out-slot of that size.
        check(
            unsafe {
                sys::ghostty_row_get(
                    row_handle,
                    sys::RowData::WRAP,
                    (&mut wraps as *mut bool).cast::<c_void>(),
                )
            },
            "row wrap",
        )?;
        Ok(match wraps {
            true => Wrap::Continues,
            false => Wrap::Ends,
        })
    }
}
