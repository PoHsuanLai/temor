//! Composing one grid row into its band of the buffer, in the spike's layer order: background,
//! `BelowBackground` images, cell backgrounds, `BelowText` images, text and decorations (and the
//! cursor), `AboveText` images.
//!
//! The band is the row's only, but its glyphs are not: the pieces of the glyphs of the rows above
//! and below that reach into the band are drawn in it (see [`super::overhang`]); a neighbour whose
//! glyphs are known to stay in its row is not even shaped.

use temor_vt::{KittyLayer, Row};

use super::overhang::{Sides, overhang_of};
use super::pixels::Target;
use crate::error::RenderError;
use crate::font::{Atlas, BUILD_ATTEMPTS, GlyphCache};
use crate::frame::{CellDraw, ImagePlacement};
use crate::scene::{Premul, Quad, QuadPaint, RowScene, SceneInput, build_row};
use crate::track::band_of;

/// Repaints the band of `row` (the full width, padding included) from the frame's `rows`, with
/// `beside` saying which neighbours' glyphs may reach into it, and says whether the row's own glyphs reach outside it. Fails when the row's glyphs do not fit
/// the atlases.
pub(crate) fn compose_row(
    target: &mut Target<'_>,
    input: &SceneInput<'_>,
    (row, beside): (Row, Sides),
    rows: &[Vec<CellDraw>],
    images: &[ImagePlacement],
    glyphs: &mut GlyphCache,
    scene: &mut RowScene,
) -> Result<bool, RenderError> {
    let ch = input.metrics.height() as i32;
    let y0 = input.origin.1 + i32::from(row.0) * ch;
    let total = u16::try_from(rows.len()).unwrap_or(u16::MAX);
    // The first and last rows' bands take in the padding beside the grid, where their glyphs
    // may overhang.
    target.band = band_of(row, total, input.origin.1, ch, target.height as i32);
    target.clear_band(Premul::opaque(input.colors.background));
    draw_images(target, input, images, KittyLayer::BelowBackground);
    let mut sides = [RowScene::default(), RowScene::default()];
    // The row with its neighbours, else (when the three rows' glyphs do not fit the atlases
    // together) the row alone, its neighbours' overhang clipped as before.
    if !build_settled(
        input,
        row,
        rows,
        glyphs,
        scene,
        &mut sides,
        Neighbours::Drawn(beside),
    ) && !build_settled(
        input,
        row,
        rows,
        glyphs,
        scene,
        &mut sides,
        Neighbours::Clipped,
    ) {
        return Err(RenderError::GlyphsDoNotFit);
    }
    for q in &scene.bg {
        target.quad(q, &glyphs.alpha, &glyphs.color);
    }
    draw_images(target, input, images, KittyLayer::BelowText);
    // Row order, as the GPU path draws: the row above's glyphs, this row's, the row below's.
    let [above, below] = &sides;
    draw_glyphs(target, &glyphs.alpha, &glyphs.color, above);
    for q in &scene.fg {
        target.quad(q, &glyphs.alpha, &glyphs.color);
    }
    draw_glyphs(target, &glyphs.alpha, &glyphs.color, below);
    draw_images(target, input, images, KittyLayer::AboveText);
    Ok(overhang_of(scene, y0, ch.unsigned_abs()))
}

/// Whether a row's band also draws its neighbours' glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Neighbours {
    /// The pieces of the rows above and below that reach into the band, for the sides whose
    /// glyphs may.
    Drawn(Sides),
    /// None: the band shows its own row only.
    Clipped,
}

/// Builds `row` into `own` and, with [`Neighbours::Drawn`], its neighbours into `sides` until
/// the glyph atlases do not clear while they are built, so every quad points at texels that are
/// still there. Returns `false` when they keep clearing.
fn build_settled(
    input: &SceneInput<'_>,
    row: Row,
    rows: &[Vec<CellDraw>],
    glyphs: &mut GlyphCache,
    own: &mut RowScene,
    sides: &mut [RowScene; 2],
    neighbours: Neighbours,
) -> bool {
    let at = usize::from(row.0);
    let own_cells = rows.get(at).map_or(&[][..], |c| c.as_slice());
    let [above, below] = match neighbours {
        Neighbours::Drawn(beside) => neighbour_cells(rows, at, beside),
        Neighbours::Clipped => [None, None],
    };
    for _ in 0..BUILD_ATTEMPTS {
        let epoch = glyphs.epoch();
        build_row(input, row, own_cells, glyphs, own);
        let [up, down] = &mut *sides;
        up.clear();
        down.clear();
        if let Some(cells) = above {
            build_row(input, Row(row.0 - 1), cells, glyphs, up);
        }
        if let Some(cells) = below {
            build_row(input, Row(row.0 + 1), cells, glyphs, down);
        }
        if glyphs.epoch() == epoch {
            return true;
        }
    }
    false
}

/// The cells of the rows above and below row `at`, for those sides of `beside` whose glyphs may
/// reach into it. A row that is off the grid has none.
fn neighbour_cells(
    rows: &[Vec<CellDraw>],
    at: usize,
    beside: Sides,
) -> [Option<&Vec<CellDraw>>; 2] {
    let reaching = |side: bool, row: Option<usize>| match side {
        true => row.and_then(|i| rows.get(i)),
        false => None,
    };
    [
        reaching(beside.above, at.checked_sub(1)),
        reaching(beside.below, at.checked_add(1)),
    ]
}

/// Draws the glyphs (not the backgrounds, rules or cursor) of a neighbouring row's scene; only
/// the parts inside the band show.
fn draw_glyphs(target: &mut Target<'_>, alpha: &Atlas, color: &Atlas, scene: &RowScene) {
    let glyph = |q: &&Quad| matches!(q.paint, QuadPaint::Mask { .. } | QuadPaint::Color { .. });
    for q in scene.fg.iter().filter(glyph) {
        target.quad(q, alpha, color);
    }
}

fn draw_images(
    target: &mut Target<'_>,
    input: &SceneInput<'_>,
    images: &[ImagePlacement],
    layer: KittyLayer,
) {
    let (cw, ch) = (input.metrics.width() as i32, input.metrics.height() as i32);
    for p in images.iter().filter(|p| p.layer == layer) {
        target.image(
            p,
            input.origin.0 + p.col * cw + p.offset.0 as i32,
            input.origin.1 + p.row * ch + p.offset.1 as i32,
        );
    }
}

#[cfg(test)]
mod tests;
