//! Styled cells of any screen row, scrollback included: what a frame's row walk says of a
//! viewport row, read by grid reference so a row the viewport does not show can be drawn too.

use std::ops::Range;

use temor_vt_sys as sys;

use super::cells::{graphemes, has_text};
use super::convert::{check, rgb};
use super::row::{cell_flag, cell_get, cell_style, cell_width, semantic};
use super::terminal::Terminal;
use crate::{Col, GridPoint, PointSpace, Rgb, StyledCell, VtError};

impl Terminal {
    /// The cells of screen row `row` (screen space), left to right, one per column.
    pub fn styled_row(&self, row: u32) -> Result<Vec<StyledCell>, VtError> {
        let palette = self.palette()?;
        self.styled_row_in(row, &palette)
    }

    /// The cells of screen rows `rows`, top to bottom.
    pub fn styled_rows(&self, rows: Range<u32>) -> Result<Vec<Vec<StyledCell>>, VtError> {
        let palette = self.palette()?;
        rows.map(|row| self.styled_row_in(row, &palette)).collect()
    }

    /// The palette the program's colours resolve through now.
    fn palette(&self) -> Result<Vec<Rgb>, VtError> {
        let mut out = [sys::ColorRgb::default(); 256];
        // SAFETY: COLOR_PALETTE writes 256 `GhosttyColorRgb`, which `out` holds.
        check(
            unsafe {
                sys::ghostty_terminal_get(
                    self.raw(),
                    sys::TerminalData::COLOR_PALETTE,
                    out.as_mut_ptr().cast(),
                )
            },
            "palette",
        )?;
        Ok(out.into_iter().map(rgb).collect())
    }

    fn styled_row_in(&self, row: u32, palette: &[Rgb]) -> Result<Vec<StyledCell>, VtError> {
        let cols = self.size().cols.0;
        let mut out = Vec::with_capacity(usize::from(cols));
        for x in 0..cols {
            let gref = self.grid_ref(GridPoint::new(PointSpace::Screen, x, row))?;
            let mut raw: sys::Cell = 0;
            let mut style = temor_vt_sys::sized!(sys::Style);
            // SAFETY: `gref` came from this live terminal; both are valid out-slots.
            unsafe {
                check(sys::ghostty_grid_ref_cell(&gref, &mut raw), "cell")?;
                check(sys::ghostty_grid_ref_style(&gref, &mut style), "style")?;
            }
            let text = match has_text(raw) {
                true => graphemes(&gref)?,
                false => String::new(),
            };
            out.push(StyledCell {
                col: Col(x),
                text,
                width: cell_width(raw),
                fg: resolve(style.fg_color, palette),
                bg: resolve(style.bg_color, palette).or_else(|| bare_background(raw, palette)),
                style: cell_style(&style, palette),
                linked: cell_flag(raw, sys::CellData::HAS_HYPERLINK),
                semantic: semantic(raw),
                selected: false,
            });
        }
        Ok(out)
    }
}

/// A style colour as channels: a palette entry through `palette`, `None` when unset.
fn resolve(c: sys::StyleColor, palette: &[Rgb]) -> Option<Rgb> {
    // SAFETY: the tag says which union member is live.
    unsafe {
        match c.tag {
            sys::StyleColorTag::RGB => Some(rgb(c.value.rgb)),
            sys::StyleColorTag::PALETTE => palette.get(usize::from(c.value.palette)).copied(),
            _ => None,
        }
    }
}

/// The colour of a cell that holds no text, only a background (an erase in a coloured SGR).
fn bare_background(raw: sys::Cell, palette: &[Rgb]) -> Option<Rgb> {
    match cell_get::<sys::CellContentTag::Type>(raw, sys::CellData::CONTENT_TAG) {
        sys::CellContentTag::BG_COLOR_PALETTE => {
            let index = cell_get::<sys::ColorPaletteIndex>(raw, sys::CellData::COLOR_PALETTE);
            palette.get(usize::from(index)).copied()
        }
        sys::CellContentTag::BG_COLOR_RGB => Some(rgb(cell_get::<sys::ColorRgb>(
            raw,
            sys::CellData::COLOR_RGB,
        ))),
        _ => None,
    }
}
