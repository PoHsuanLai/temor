//! `RowView`: one row of a frame and the walk over its cells.

use temor_vt_sys as sys;

use super::convert::{check, rgb};
use crate::{
    CellStyle, CellView, CellWidth, Col, ColorRef, Rgb, Row, RowId, SemanticMark, StyleFlags,
    Underline, VtError,
};

/// One row of a [`crate::Frame`], valid during the row walk.
pub struct RowView<'a> {
    iter: sys::RenderStateRowIterator,
    cells: sys::RenderStateRowCells,
    text: &'a mut Vec<u8>,
    palette: &'a [Rgb],
}

impl std::fmt::Debug for RowView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowView")
            .field("row", &self.row())
            .finish_non_exhaustive()
    }
}

impl<'a> RowView<'a> {
    pub(super) fn new(
        iter: sys::RenderStateRowIterator,
        cells: sys::RenderStateRowCells,
        text: &'a mut Vec<u8>,
        palette: &'a [Rgb],
    ) -> Self {
        Self {
            iter,
            cells,
            text,
            palette,
        }
    }

    /// The row's position from the top of the viewport.
    pub fn row(&self) -> Row {
        let mut y: i32 = 0;
        // SAFETY: the iterator is positioned on a row; VIEWPORT_Y writes an `int32_t`.
        unsafe {
            sys::ghostty_render_state_row_get(
                self.iter,
                sys::RenderStateRowData::VIEWPORT_Y,
                (&raw mut y).cast(),
            )
        };
        Row(u16::try_from(y).unwrap_or(0))
    }

    /// Whether the row changed since the frame was last marked drawn.
    pub fn dirty(&self) -> bool {
        let mut dirty = true;
        // SAFETY: DIRTY writes a `bool`.
        unsafe {
            sys::ghostty_render_state_row_get(
                self.iter,
                sys::RenderStateRowData::DIRTY,
                (&raw mut dirty).cast(),
            )
        };
        dirty
    }

    /// The row's identity across frames.
    pub fn id(&self) -> RowId {
        let mut id = sys::RenderStateRowId::default();
        // SAFETY: ID writes a `GhosttyRenderStateRowId`.
        unsafe {
            sys::ghostty_render_state_row_get(
                self.iter,
                sys::RenderStateRowData::ID,
                (&raw mut id).cast(),
            )
        };
        RowId(id.bits)
    }

    /// Calls `f` for each cell, left to right.
    pub fn cells(&mut self, mut f: impl FnMut(&CellView<'_>)) -> Result<(), VtError> {
        let mut cells = self.cells;
        // SAFETY: CELLS fills the preallocated cell cursor for the current row.
        check(
            unsafe {
                sys::ghostty_render_state_row_get(
                    self.iter,
                    sys::RenderStateRowData::CELLS,
                    (&raw mut cells).cast(),
                )
            },
            "cells",
        )?;
        let mut col = 0u16;
        // SAFETY: the cursor was just filled from a state that is not updated during the walk.
        while unsafe { sys::ghostty_render_state_row_cells_next(cells) } {
            let cell = read_cell(cells, Col(col), self.text, self.palette)?;
            f(&cell);
            col = col.saturating_add(1);
        }
        Ok(())
    }
}

/// Reads the cell under the cursor. Its text borrows `text`.
fn read_cell<'t>(
    cells: sys::RenderStateRowCells,
    col: Col,
    text: &'t mut Vec<u8>,
    palette: &[Rgb],
) -> Result<CellView<'t>, VtError> {
    use sys::RenderStateRowCellsData as D;
    let mut raw: sys::Cell = 0;
    let mut style = temor_vt_sys::sized!(sys::Style);
    let mut selected = false;
    let (mut fg, mut bg) = (sys::ColorRgb::default(), sys::ColorRgb::default());
    // SAFETY: the cursor is positioned on a cell; each key writes the type documented for it.
    let (has_fg, has_bg) = unsafe {
        check(
            sys::ghostty_render_state_row_cells_get(cells, D::RAW, (&raw mut raw).cast()),
            "cell",
        )?;
        check(
            sys::ghostty_render_state_row_cells_get(cells, D::STYLE, (&raw mut style).cast()),
            "style",
        )?;
        sys::ghostty_render_state_row_cells_get(cells, D::SELECTED, (&raw mut selected).cast());
        (
            sys::ghostty_render_state_row_cells_get(cells, D::FG_COLOR, (&raw mut fg).cast())
                == sys::Result::SUCCESS,
            sys::ghostty_render_state_row_cells_get(cells, D::BG_COLOR, (&raw mut bg).cast())
                == sys::Result::SUCCESS,
        )
    };
    // A grapheme libghostty cannot hand over shows as a replacement character; the rest of
    // the row still draws.
    if read_text(cells, text).is_err() {
        text.clear();
        text.extend_from_slice("\u{fffd}".as_bytes());
    }
    Ok(CellView {
        col,
        text: std::str::from_utf8(text).unwrap_or("\u{fffd}"),
        width: cell_width(raw),
        fg: has_fg.then(|| rgb(fg)),
        bg: has_bg.then(|| rgb(bg)),
        style: cell_style(&style, palette),
        linked: cell_flag(raw, sys::CellData::HAS_HYPERLINK),
        semantic: semantic(raw),
        selected,
    })
}

/// Writes the cell's grapheme cluster as UTF-8 into `text`, growing it when needed.
fn read_text(cells: sys::RenderStateRowCells, text: &mut Vec<u8>) -> Result<(), VtError> {
    text.clear();
    loop {
        let mut buf = sys::Buffer {
            ptr: text.as_mut_ptr(),
            cap: text.capacity(),
            len: 0,
        };
        // SAFETY: `buf` describes `text`'s spare capacity, which libghostty may fill up to `cap`.
        let code = unsafe {
            sys::ghostty_render_state_row_cells_get(
                cells,
                sys::RenderStateRowCellsData::GRAPHEMES_UTF8,
                (&raw mut buf).cast(),
            )
        };
        if code == sys::Result::OUT_OF_SPACE {
            text.reserve(buf.len.max(text.capacity() * 2));
            continue;
        }
        check(code, "grapheme")?;
        // SAFETY: on success libghostty initialised `buf.len <= cap` bytes.
        unsafe { text.set_len(buf.len.min(text.capacity())) };
        return Ok(());
    }
}

pub(super) fn cell_get<T: Default>(raw: sys::Cell, data: sys::CellData::Type) -> T {
    let mut out = T::default();
    // SAFETY: `ghostty_cell_get` decodes the packed value; callers pass the type documented for
    // `data`.
    unsafe { sys::ghostty_cell_get(raw, data, (&raw mut out).cast()) };
    out
}

pub(super) fn cell_flag(raw: sys::Cell, data: sys::CellData::Type) -> bool {
    cell_get::<bool>(raw, data)
}

pub(super) fn cell_width(raw: sys::Cell) -> CellWidth {
    match cell_get::<sys::CellWide::Type>(raw, sys::CellData::WIDE) {
        sys::CellWide::WIDE => CellWidth::Wide,
        sys::CellWide::SPACER_TAIL => CellWidth::SpacerTail,
        sys::CellWide::SPACER_HEAD => CellWidth::SpacerHead,
        _ => CellWidth::Narrow,
    }
}

pub(super) fn semantic(raw: sys::Cell) -> SemanticMark {
    match cell_get::<sys::CellSemanticContent::Type>(raw, sys::CellData::SEMANTIC_CONTENT) {
        sys::CellSemanticContent::INPUT => SemanticMark::Input,
        sys::CellSemanticContent::PROMPT => SemanticMark::Prompt,
        _ => SemanticMark::Output,
    }
}

pub(super) fn cell_style(s: &sys::Style, palette: &[Rgb]) -> CellStyle {
    let flags = [
        (s.bold, StyleFlags::BOLD),
        (s.italic, StyleFlags::ITALIC),
        (s.faint, StyleFlags::FAINT),
        (s.blink, StyleFlags::BLINK),
        (s.inverse, StyleFlags::INVERSE),
        (s.invisible, StyleFlags::INVISIBLE),
        (s.strikethrough, StyleFlags::STRIKETHROUGH),
        (s.overline, StyleFlags::OVERLINE),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .fold(StyleFlags::default(), |acc, (_, flag)| acc.with(flag));
    let underline = match s.underline {
        sys::SgrUnderline::SINGLE => Underline::Single,
        sys::SgrUnderline::DOUBLE => Underline::Double,
        sys::SgrUnderline::CURLY => Underline::Curly,
        sys::SgrUnderline::DOTTED => Underline::Dotted,
        sys::SgrUnderline::DASHED => Underline::Dashed,
        _ => Underline::None,
    };
    let color_ref = |c: sys::StyleColor| {
        // SAFETY: the tag says which union member is live.
        unsafe {
            match c.tag {
                sys::StyleColorTag::RGB => ColorRef::Rgb(rgb(c.value.rgb)),
                sys::StyleColorTag::PALETTE => ColorRef::Palette(c.value.palette),
                _ => ColorRef::Unset,
            }
        }
    };
    let c = s.underline_color;
    // SAFETY: the tag says which union member is live.
    let underline_color = unsafe {
        match c.tag {
            sys::StyleColorTag::RGB => Some(rgb(c.value.rgb)),
            sys::StyleColorTag::PALETTE => palette.get(usize::from(c.value.palette)).copied(),
            _ => None,
        }
    };
    CellStyle {
        flags,
        underline,
        underline_color,
        fg_ref: color_ref(s.fg_color),
        underline_ref: color_ref(s.underline_color),
    }
}
