//! Composing one grid row into its band of the buffer, in the spike's layer order: background,
//! `BelowBackground` images, cell backgrounds, `BelowText` images, text and decorations (and the
//! cursor), `AboveText` images.
//!
//! The band is the row's only, but its glyphs are not: the pieces of the glyphs of the rows above
//! and below that reach into the band are drawn in it (see [`super::overhang`]); a neighbour whose
//! glyphs are known to stay in its row is not even shaped.

use std::convert::Infallible;

use temor_vt::{KittyLayer, Row};

use super::overhang::{Overhangs, Sides, compose_with_neighbours, overhang_of};
use super::pixels::Target;
use crate::font::GlyphCache;
use crate::frame::{CellDraw, ImagePlacement};
use crate::scene::{Premul, Quad, QuadPaint, RowScene, SceneInput, build_row};
use crate::track::band_of;

/// Composes `rows` of the frame's `cells` into `target`, and the rows beside them that their
/// glyphs, or the rows' own, reach (see [`compose_with_neighbours`]). The rows composed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compose_rows(
    target: &mut Target<'_>,
    input: &SceneInput<'_>,
    rows: &[Row],
    cells: &[Vec<CellDraw>],
    images: &[ImagePlacement],
    overhangs: &mut Overhangs,
    glyphs: &mut GlyphCache,
    scene: &mut RowScene,
) -> Vec<Row> {
    let total = u16::try_from(cells.len()).unwrap_or(u16::MAX);
    overhangs.fit(total);
    let composed = compose_with_neighbours(rows, total, overhangs, |row, beside| {
        Ok::<_, Infallible>(compose_row(
            target,
            input,
            (row, beside),
            cells,
            images,
            glyphs,
            scene,
        ))
    });
    composed.unwrap_or_else(|never| match never {})
}

/// Repaints the band of `row` (the full width, padding included) from the frame's `rows`, with
/// `beside` saying which neighbours' glyphs may reach into it, and says whether the row's own
/// glyphs reach outside it.
fn compose_row(
    target: &mut Target<'_>,
    input: &SceneInput<'_>,
    (row, beside): (Row, Sides),
    rows: &[Vec<CellDraw>],
    images: &[ImagePlacement],
    glyphs: &mut GlyphCache,
    scene: &mut RowScene,
) -> bool {
    let ch = input.metrics.height() as i32;
    let y0 = input.origin.1 + i32::from(row.0) * ch;
    let total = u16::try_from(rows.len()).unwrap_or(u16::MAX);
    // The first and last rows' bands take in the padding beside the grid, where their glyphs
    // may overhang.
    target.band = band_of(row, total, input.origin.1, ch, target.height as i32);
    target.clear_band(Premul::opaque(input.colors.background));
    draw_images(target, input, images, KittyLayer::BelowBackground);
    let at = usize::from(row.0);
    build_row(
        input,
        row,
        rows.get(at).map_or(&[][..], |c| c),
        glyphs,
        scene,
    );
    let [above, below] = neighbour_cells(rows, at, beside).map(|cells| {
        let mut side = RowScene::default();
        if let Some((n, cells)) = cells {
            build_row(input, n, cells, glyphs, &mut side);
        }
        side
    });
    for q in &scene.bg {
        target.quad(q, glyphs);
    }
    draw_images(target, input, images, KittyLayer::BelowText);
    // Row order, as the GPU path draws: the row above's glyphs, this row's, the row below's.
    draw_glyphs(target, glyphs, &above);
    for q in &scene.fg {
        target.quad(q, glyphs);
    }
    draw_glyphs(target, glyphs, &below);
    draw_images(target, input, images, KittyLayer::AboveText);
    overhang_of(scene, y0, ch.unsigned_abs())
}

/// The rows above and below row `at` and their cells, for those sides of `beside` whose glyphs
/// may reach into it. A row that is off the grid has none.
fn neighbour_cells(
    rows: &[Vec<CellDraw>],
    at: usize,
    beside: Sides,
) -> [Option<(Row, &Vec<CellDraw>)>; 2] {
    let reaching = |side: bool, row: Option<usize>| match side {
        true => row.and_then(|i| Some((Row(u16::try_from(i).ok()?), rows.get(i)?))),
        false => None,
    };
    [
        reaching(beside.above, at.checked_sub(1)),
        reaching(beside.below, at.checked_add(1)),
    ]
}

/// Draws the glyphs (not the backgrounds, rules or cursor) of a neighbouring row's scene; only
/// the parts inside the band show.
fn draw_glyphs(target: &mut Target<'_>, glyphs: &GlyphCache, scene: &RowScene) {
    let glyph = |q: &&Quad| matches!(q.paint, QuadPaint::Mask { .. } | QuadPaint::Color { .. });
    for q in scene.fg.iter().filter(glyph) {
        target.quad(q, glyphs);
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
