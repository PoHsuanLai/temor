//! Turning one glyph into pixels: outlines through swash (an alpha mask), colour glyphs through
//! vello_cpu (COLR) or the platform's bitmap source, fitted to the cell box.

use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Angle, Format, Transform};

use super::face::{Embolden, Face, Skew};
use crate::platform::ColorBitmap;

/// A coverage mask placed relative to the pen: `left` right of it, `top` above the baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaskImage {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Premultiplied RGBA filling a cell box exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoxImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The outline of `glyph` at `px`, thickened or slanted as asked, hinted.
pub(crate) fn outline(
    ctx: &mut ScaleContext,
    face: &Face,
    glyph: u16,
    px: f32,
    synth: (Embolden, Skew),
) -> Option<MaskImage> {
    let mut scaler = ctx
        .builder(face.font())
        .size(px)
        .hint(true)
        .variations(face.variations.iter())
        .build();
    let mut render = Render::new(&[Source::Outline]);
    render.format(Format::Alpha);
    if synth.1 == Skew::Yes {
        render.transform(Some(Transform::skew(
            Angle::from_degrees(12.0),
            Angle::ZERO,
        )));
    }
    if synth.0 == Embolden::Yes {
        render.embolden((px / 24.0).max(0.5));
    }
    let image = render.render(&mut scaler, glyph)?;
    let (width, height) = (image.placement.width, image.placement.height);
    if width == 0 || height == 0 || image.content != Content::Mask {
        return None;
    }
    Some(MaskImage {
        left: image.placement.left,
        top: image.placement.top,
        width,
        height,
        data: image.data,
    })
}

/// Paints a COLR glyph (v0 or v1) through vello_cpu, scaled down to fit `box_w` x `box_h` and
/// centred. `None` when nothing was painted.
pub(crate) fn colr(face: &Face, glyph: u16, px: f32, box_w: u32, box_h: u32) -> Option<BoxImage> {
    use vello_cpu::{Glyph, Level, Pixmap, RenderContext, RenderSettings, Resources};

    let (w16, h16) = (u16::try_from(box_w).ok()?, u16::try_from(box_h).ok()?);
    if w16 == 0 || h16 == 0 {
        return None;
    }
    let font = face.font();
    let metrics = font.metrics(&[]).scale(px);
    let advance = font
        .glyph_metrics(&[])
        .scale(px)
        .advance_width(glyph)
        .max(1.0);
    let tall = (metrics.ascent + metrics.descent).max(1.0);
    let fit = (box_w as f32 / advance).min(box_h as f32 / tall).min(1.0);
    let x = (box_w as f32 - advance * fit) / 2.0;
    let baseline = (box_h as f32 - tall * fit) / 2.0 + metrics.ascent * fit;
    // Baseline SIMD and no worker threads: deterministic, and the library spawns nothing.
    let settings = RenderSettings {
        level: Level::baseline(),
        num_threads: 0,
    };
    let mut ctx = RenderContext::new_with(w16, h16, settings);
    let mut resources = Resources::new();
    ctx.set_paint(vello_cpu::color::palette::css::BLACK);
    ctx.glyph_run(&mut resources, &face.font_data())
        .font_size(px * fit)
        .hint(false)
        .fill_glyphs(std::iter::once(Glyph {
            id: u32::from(glyph),
            x,
            y: baseline,
        }));
    ctx.flush();
    let mut target = Pixmap::new(w16, h16);
    ctx.render(&mut target, &mut resources);
    let rgba = target.data_as_u8_slice().to_vec();
    rgba.as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[3] != 0)
        .then_some(BoxImage {
            width: box_w,
            height: box_h,
            rgba,
        })
}

/// A bitmap glyph premultiplied, box-filtered down to fit `box_w` x `box_h` and centred.
pub(crate) fn fit_bitmap(src: &ColorBitmap, box_w: u32, box_h: u32) -> BoxImage {
    let (w, h) = (src.width.max(1), src.height.max(1));
    let s = (box_w as f32 / w as f32)
        .min(box_h as f32 / h as f32)
        .min(1.0);
    let w2 = ((w as f32 * s).round() as u32).clamp(1, box_w.max(1));
    let h2 = ((h as f32 * s).round() as u32).clamp(1, box_h.max(1));
    let (ox, oy) = ((box_w - w2) / 2, (box_h - h2) / 2);
    let mut rgba = vec![0u8; (box_w * box_h * 4) as usize];
    for y in 0..h2 {
        for x in 0..w2 {
            let (x0, x1) = (x * w / w2, ((x + 1) * w / w2).max(x * w / w2 + 1).min(w));
            let (y0, y1) = (y * h / h2, ((y + 1) * h / h2).max(y * h / h2 + 1).min(h));
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * w + sx) * 4) as usize;
                    let Some(p) = src.rgba.get(i..i + 4) else {
                        continue;
                    };
                    let a = u32::from(p[3]);
                    for c in 0..3 {
                        acc[c] += u32::from(p[c]) * a / 255;
                    }
                    acc[3] += a;
                    n += 1;
                }
            }
            let o = (((oy + y) * box_w + ox + x) * 4) as usize;
            for c in 0..4 {
                rgba[o + c] = (acc[c] / n.max(1)) as u8;
            }
        }
    }
    BoxImage {
        width: box_w,
        height: box_h,
        rgba,
    }
}
