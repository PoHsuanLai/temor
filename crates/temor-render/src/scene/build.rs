//! Turning one row of cells into quads, shared by the CPU and GPU renderers.

use temor_vt::{Col, Row, StyleFlags, Underline};

use super::colors::{CellColors, UnderCursor, cell_colors};
use super::cursor::{CursorDraw, CursorStyle};
use super::model::{Premul, Quad, QuadPaint, RowScene};
use crate::bands::{RowBand, RowBox, RowWeight, look_of, push_quads};
use crate::font::{
    BUILD_ATTEMPTS, Decoration, FaceStyle, GlyphCache, GlyphKind, RunGlyph, Slant, Weight,
};
use crate::frame::{CellDraw, TermColors};
use crate::metrics::{CellMetrics, CellPos};
use crate::palette::Palette;
use crate::renderer::SelectionOverlay;

/// Everything about the pane a row needs to be built.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SceneInput<'a> {
    pub metrics: CellMetrics,
    /// The grid's top-left in the output, in device pixels.
    pub origin: (i32, i32),
    pub cols: u16,
    pub palette: &'a Palette,
    pub colors: &'a TermColors,
    pub cursor: Option<CursorDraw>,
    pub selection: Option<SelectionOverlay>,
    pub bands: &'a [RowBand],
    /// The pane's width and the stripe's, in device pixels.
    pub pane_width: u32,
    pub stripe_width: u32,
}

/// Builds `row` until the glyph atlases do not clear while it is built, so every quad points at
/// texels that are still there. Returns `false` when they keep clearing; the caller then drops
/// the frame.
pub(crate) fn build_row_settled(
    input: &SceneInput<'_>,
    row: Row,
    cells: &[CellDraw],
    glyphs: &mut GlyphCache,
    out: &mut RowScene,
) -> bool {
    for _ in 0..BUILD_ATTEMPTS {
        let epoch = glyphs.epoch();
        build_row(input, row, cells, glyphs, out);
        if glyphs.epoch() == epoch {
            return true;
        }
    }
    false
}

/// Builds `row`'s quads into `out` (cleared first).
pub(crate) fn build_row(
    input: &SceneInput<'_>,
    row: Row,
    cells: &[CellDraw],
    glyphs: &mut GlyphCache,
    out: &mut RowScene,
) {
    out.clear();
    let m = input.metrics;
    let (cw, ch) = (m.width(), m.height());
    let y0 = offset(input.origin.1, i64::from(row.0), ch);
    let x_of = |col: Col| offset(input.origin.0, i64::from(col.0), cw);
    if let Some((c0, c1)) = input.selection.and_then(|s| s.cols_on(row, input.cols)) {
        let w = (u32::from(c1.0) - u32::from(c0.0) + 1) * cw;
        out.bg
            .push(Quad::solid(x_of(c0), y0, w, ch, input.palette.selection));
    }
    let look = look_of(input.bands, row);
    let cursor = input.cursor.filter(|c| c.row == row);
    let block = cursor.filter(|c| c.style == CursorStyle::Block);
    for cell in cells {
        let under = if block.is_some_and(|c| c.covers(cell.col)) {
            UnderCursor::Block
        } else {
            UnderCursor::No
        };
        let pos = CellPos { col: cell.col, row };
        let colors = cell_colors(
            cell,
            pos,
            input.colors,
            input.palette,
            input.selection,
            under,
        );
        let (x0, span_w) = (x_of(cell.col), cw * u32::from(cell.span));
        if let Some(bg) = colors.bg {
            out.bg.push(Quad::solid(x0, y0, span_w, ch, bg));
        }
        if !cell.style.flags.contains(StyleFlags::INVISIBLE) {
            text(cell, &colors, look.weight, (x0, y0), glyphs, out);
            decorations(cell, &colors, (x0, y0), &m, glyphs, out);
        }
    }
    let at = RowBox {
        y: y0,
        height: ch,
        pane_width: input.pane_width,
        stripe_width: input.stripe_width,
    };
    push_quads(&look, at, out);
    if let Some(c) = cursor {
        cursor_quads(&c, (x_of(c.col), y0), &m, out);
    }
}

fn text(
    cell: &CellDraw,
    colors: &CellColors,
    weight: RowWeight,
    at: (i32, i32),
    glyphs: &mut GlyphCache,
    out: &mut RowScene,
) {
    if cell.text.is_blank() {
        return;
    }
    let flags = cell.style.flags;
    let style = FaceStyle {
        weight: if flags.contains(StyleFlags::BOLD) || weight.bolds(cell.col) {
            Weight::Bold
        } else {
            Weight::Regular
        },
        slant: if flags.contains(StyleFlags::ITALIC) {
            Slant::Italic
        } else {
            Slant::Upright
        },
    };
    let mut buf = [0u8; 4];
    let run = glyphs.run(cell.text.as_str(&mut buf), style, cell.span);
    for g in run.iter() {
        out.fg.push(glyph_quad(g, at, Premul::opaque(colors.fg)));
    }
}

fn glyph_quad(g: &RunGlyph, at: (i32, i32), color: Premul) -> Quad {
    Quad {
        x: at.0 + g.dx,
        y: at.1 + g.dy,
        w: g.slot.w,
        h: g.slot.h,
        paint: match g.kind {
            GlyphKind::Mask => QuadPaint::Mask {
                slot: g.slot,
                color,
            },
            GlyphKind::Color => QuadPaint::Color { slot: g.slot },
        },
    }
}

fn decorations(
    cell: &CellDraw,
    colors: &CellColors,
    (x0, y0): (i32, i32),
    m: &CellMetrics,
    glyphs: &mut GlyphCache,
    out: &mut RowScene,
) {
    let (cw, ch, t) = (m.width(), m.height(), m.thickness.0.max(1));
    let w = cw * u32::from(cell.span);
    let ul = colors.underline;
    let bar = |y: u32, color| Quad::solid(x0, y0 + y.min(ch.saturating_sub(t)) as i32, w, t, color);
    let masked = |d: Decoration, glyphs: &mut GlyphCache, out: &mut RowScene| {
        if let Some(g) = glyphs.decoration(d) {
            for i in 0..i32::from(cell.span) {
                let x = offset(x0, i64::from(i), cw);
                out.fg.push(glyph_quad(&g, (x, y0), Premul::opaque(ul)));
            }
        }
    };
    match cell.style.underline {
        Underline::None => {}
        Underline::Single => out.fg.push(bar(m.underline.0, ul)),
        Underline::Double => {
            let top = m.underline.0.min(ch.saturating_sub(3 * t));
            out.fg.push(bar(top, ul));
            out.fg.push(bar(top + 2 * t, ul));
        }
        Underline::Curly => masked(Decoration::Curly, glyphs, out),
        Underline::Dotted => masked(Decoration::Dotted, glyphs, out),
        Underline::Dashed => masked(Decoration::Dashed, glyphs, out),
    }
    let flags = cell.style.flags;
    if flags.contains(StyleFlags::STRIKETHROUGH) {
        out.fg.push(bar(m.strikeout.0, colors.fg));
    }
    if flags.contains(StyleFlags::OVERLINE) {
        out.fg.push(bar(0, colors.fg));
    }
}

/// `origin` moved `n` steps of `step` device pixels, saturating at the `i32` range so a huge
/// grid or image cannot overflow.
fn offset(origin: i32, n: i64, step: u32) -> i32 {
    let v = i64::from(origin) + n * i64::from(step);
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

fn cursor_quads(c: &CursorDraw, (x0, y0): (i32, i32), m: &CellMetrics, out: &mut RowScene) {
    let (w, h) = (m.width() * u32::from(c.span), m.height());
    let t = m.thickness.0.max(1);
    match c.style {
        // Drawn over the cell's background, under its text (which takes the cell's colour).
        CursorStyle::Block => out.bg.push(Quad::solid(x0, y0, w, h, c.color)),
        CursorStyle::Bar => out
            .fg
            .push(Quad::solid(x0, y0, t.max(2).min(w), h, c.color)),
        CursorStyle::Underline => {
            let y = m.underline.0.min(h.saturating_sub(t));
            out.fg.push(Quad::solid(
                x0,
                y0 + y as i32,
                w,
                t.max(2).min(h.saturating_sub(y)),
                c.color,
            ));
        }
        CursorStyle::Hollow => {
            let (wi, hi) = (w as i32, h as i32);
            out.fg.push(Quad::solid(x0, y0, w, t, c.color));
            out.fg
                .push(Quad::solid(x0, y0 + hi - t as i32, w, t, c.color));
            out.fg.push(Quad::solid(x0, y0, t, h, c.color));
            out.fg
                .push(Quad::solid(x0 + wi - t as i32, y0, t, h, c.color));
        }
    }
}
