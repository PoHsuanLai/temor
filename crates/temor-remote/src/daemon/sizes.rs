//! The sizes a session is given, as each of its three users wants them: the wire's, the
//! pseudo-terminal's (which carries pixels) and the screen's.

use temor_pty::{Cols, GridSize, Px, Rows};
use temor_vt::{CellPx, GridSize as ScreenSize};
use temor_wire::units::{CellPx as CellSize, Size};

use crate::error::Error;

pub(super) fn check(size: Size) -> Result<Size, Error> {
    if size.cols.0 == 0 || size.rows.0 == 0 {
        Err(Error::BadSize)
    } else {
        Ok(size)
    }
}

/// A cell size of zero would make every pixel report zero; fall back to the default instead.
pub(super) fn usable(cell: CellSize) -> CellSize {
    if cell.width.0 == 0 || cell.height.0 == 0 {
        CellSize::DEFAULT
    } else {
        cell
    }
}

/// The terminal's size with the text area in pixels, as `TIOCSWINSZ` carries it.
pub(super) fn pty_size(size: Size, cell: CellSize) -> GridSize {
    GridSize {
        cols: Cols(size.cols.0),
        rows: Rows(size.rows.0),
        width: Px(size.cols.0.saturating_mul(cell.width.0)),
        height: Px(size.rows.0.saturating_mul(cell.height.0)),
    }
}

pub(super) fn screen_cell(cell: CellSize) -> CellPx {
    CellPx::new(u32::from(cell.width.0), u32::from(cell.height.0))
}

pub(super) fn screen_size(size: Size) -> ScreenSize {
    ScreenSize::new(size.cols.0, size.rows.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::units::{Cols as WireCols, Pixels, Rows as WireRows};

    fn size(cols: u16, rows: u16) -> Size {
        Size {
            cols: WireCols(cols),
            rows: WireRows(rows),
        }
    }

    fn cell(width: u16, height: u16) -> CellSize {
        CellSize {
            width: Pixels(width),
            height: Pixels(height),
        }
    }

    #[test]
    fn the_terminal_is_told_the_text_area_in_pixels() {
        let cases = [
            (size(80, 24), cell(8, 16), (640, 384)),
            (size(100, 30), cell(10, 20), (1000, 600)),
            // Sizes that do not fit a u16 stop at the top instead of wrapping.
            (size(60_000, 24), cell(8, 16), (u16::MAX, 384)),
            // A cell of no size is read as the default.
            (size(80, 24), cell(0, 20), (640, 384)),
        ];
        for (size, cell, (width, height)) in cases {
            let grid = pty_size(size, usable(cell));
            assert_eq!(
                (grid.width, grid.height),
                (Px(width), Px(height)),
                "{size:?} {cell:?}"
            );
        }
    }
}
