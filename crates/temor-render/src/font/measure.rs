//! Cell metrics from the primary face, in whole device pixels.

use temor_vt::{CellPx, Px};

use super::face::Face;
use crate::metrics::CellMetrics;

/// The cell of `face` at `px` device pixels: the advance of `0` wide, ascent plus descent plus
/// line gap tall, every offset rounded so lines land on whole pixels at any scale.
pub(crate) fn cell_metrics(face: &Face, px: f32) -> CellMetrics {
    let font = face.font();
    let m = font.metrics(&[]).scale(px);
    let zero = font.charmap().map('0');
    let advance = font.glyph_metrics(&[]).scale(px).advance_width(zero);
    let advance = if advance > 0.0 { advance } else { px * 0.6 };
    let width = (advance.round() as u32).max(1);
    let ascent = m.ascent.ceil().max(1.0);
    let height = ((ascent + m.descent.max(0.0).ceil() + m.leading.max(0.0)).round() as u32).max(2);
    let baseline = (ascent as u32).min(height - 1);
    let thickness = (m.stroke_size.round() as u32).clamp(1, (height / 4).max(1));
    let underline = (baseline as f32 - m.underline_offset).round().max(0.0) as u32;
    let strike_top = if m.strikeout_offset > 0.0 {
        m.strikeout_offset
    } else {
        m.x_height.max(px * 0.5) / 2.0
    };
    let strikeout = (baseline as f32 - strike_top).round().max(0.0) as u32;
    CellMetrics {
        cell: CellPx::new(width, height),
        baseline: Px(baseline),
        underline: Px(underline.min(height - thickness)),
        strikeout: Px(strikeout.min(height - thickness)),
        thickness: Px(thickness),
    }
}
