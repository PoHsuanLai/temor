//! A cell's final colours: the terminal's defaults, inverse, selection, faint and the block
//! cursor.

use temor_vt::{Rgb, Selected, StyleFlags};

use crate::frame::{CellDraw, TermColors};
use crate::metrics::CellPos;
use crate::palette::Palette;
use crate::renderer::SelectionOverlay;

/// Halfway between `a` and `b` (faint text).
pub(crate) fn mix(a: Rgb, b: Rgb) -> Rgb {
    let m = |x: u8, y: u8| ((u16::from(x) + u16::from(y)) / 2) as u8;
    Rgb::new(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b))
}

/// The colours one cell draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellColors {
    pub fg: Rgb,
    /// `None`: the pane's background shows through.
    pub bg: Option<Rgb>,
    pub underline: Rgb,
}

/// Whether a block cursor covers the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnderCursor {
    No,
    Block,
}

/// Resolves `cell`'s colours at `pos`.
pub(crate) fn cell_colors(
    cell: &CellDraw,
    pos: CellPos,
    colors: &TermColors,
    palette: &Palette,
    selection: Option<SelectionOverlay>,
    cursor: UnderCursor,
) -> CellColors {
    let flags = cell.style.flags;
    let fg = cell.fg.unwrap_or(colors.foreground);
    let bg = cell.bg;
    let (mut fg, mut bg) = if flags.contains(StyleFlags::INVERSE) {
        (bg.unwrap_or(colors.background), Some(fg))
    } else {
        (fg, bg)
    };
    let selected = cell.selected == Selected::Yes || selection.is_some_and(|s| s.contains(pos));
    if selected {
        bg = Some(palette.selection);
    }
    if flags.contains(StyleFlags::FAINT) {
        fg = mix(fg, bg.unwrap_or(colors.background));
    }
    let underline = cell.style.underline_color.unwrap_or(fg);
    if cursor == UnderCursor::Block {
        // Text under a block cursor takes the cell's background so it stays readable.
        fg = bg.unwrap_or(colors.background);
    }
    CellColors { fg, bg, underline }
}
