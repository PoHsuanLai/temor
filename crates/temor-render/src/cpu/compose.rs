//! Composing one grid row into its band of the buffer, in the spike's layer order: background,
//! `BelowBackground` images, cell backgrounds, `BelowText` images, text and decorations (and the
//! cursor), `AboveText` images.

use temor_vt::{KittyLayer, Row};

use super::pixels::Target;
use crate::font::GlyphCache;
use crate::frame::{CellDraw, ImagePlacement};
use crate::scene::{Premul, RowScene, SceneInput, build_row};

/// Repaints the band of `row` (the full width, padding included).
pub(crate) fn compose_row(
    target: &mut Target<'_>,
    input: &SceneInput<'_>,
    row: Row,
    cells: &[CellDraw],
    images: &[ImagePlacement],
    glyphs: &mut GlyphCache,
    scene: &mut RowScene,
) {
    let ch = input.metrics.height() as i32;
    let y0 = input.origin.1 + i32::from(row.0) * ch;
    target.band = (y0, y0 + ch);
    target.clear_band(Premul::opaque(input.colors.background));
    draw_images(target, input, images, KittyLayer::BelowBackground);
    let epoch = glyphs.epoch();
    build_row(input, row, cells, glyphs, scene);
    if glyphs.epoch() != epoch {
        // The atlas was cleared while building: glyphs placed before that are gone.
        build_row(input, row, cells, glyphs, scene);
    }
    for q in &scene.bg {
        target.quad(q, &glyphs.alpha, &glyphs.color);
    }
    draw_images(target, input, images, KittyLayer::BelowText);
    for q in &scene.fg {
        target.quad(q, &glyphs.alpha, &glyphs.color);
    }
    draw_images(target, input, images, KittyLayer::AboveText);
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
