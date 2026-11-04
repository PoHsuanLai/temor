//! Building a cluster's glyphs: choose the face, shape, and name and place each glyph; and
//! drawing a glyph from its name.

use super::boxdraw;
use super::cache::{GlyphCache, GlyphImage, GlyphKey, Placed, RunGlyph, Special, run_glyph};
use super::config::{FaceStyle, Slant, Weight};
use super::decor::decoration_mask;
use super::face::{ColorFormat, Face, FaceRole, Synth};
use super::raster::{colr, fit_bitmap, outline};
use crate::platform::BitmapEmojiSource;

impl GlyphCache {
    pub(super) fn build_run(&mut self, text: &str, style: FaceStyle, span: u8) -> Vec<RunGlyph> {
        let m = self.metrics;
        let (box_w, box_h) = (m.width() * u32::from(span.max(1)), m.height());
        let mut chars = text.chars();
        if let (Some(c), None) = (chars.next(), chars.next())
            && boxdraw::is_procedural(c)
        {
            let key = GlyphKey::Special(Special::Box(c));
            return self.glyph(key, (0, 0)).into_iter().collect();
        }
        let id = self.fonts.face_for(text, style);
        let face = self.fonts.face(id).clone();
        let (placed, advance) = self.shape(&face, text);
        let mut out = Vec::with_capacity(placed.len());
        if face.color != ColorFormat::None {
            let each = box_w / placed.len().max(1) as u32;
            let mut x = 0;
            for &(glyph, _) in &placed {
                let key = GlyphKey::Color {
                    face: id,
                    glyph,
                    size: (each, box_h),
                };
                out.extend(self.glyph(key, (x, 0)));
                x += each as i32;
            }
            if !out.is_empty() {
                return out;
            }
        }
        let synth = match face.role {
            FaceRole::Primary => face.synth,
            FaceRole::Fallback => Synth {
                embolden: style.weight == Weight::Bold,
                skew: style.slant == Slant::Italic,
            },
        };
        // A fallback face's glyphs are centred in the cells they span.
        let shift = match face.role {
            FaceRole::Primary => 0,
            FaceRole::Fallback => ((box_w as f32 - advance) / 2.0).round() as i32,
        };
        for (glyph, x) in placed {
            let key = GlyphKey::Mask {
                face: id,
                glyph,
                synth,
            };
            out.extend(self.glyph(key, (x.round() as i32 + shift, 0)));
        }
        out
    }

    /// `key` as a run's glyph, moved by `by`; none when it draws nothing.
    fn glyph(&mut self, key: GlyphKey, by: (i32, i32)) -> Option<RunGlyph> {
        self.placed(key).map(|placed| run_glyph(key, placed, by))
    }

    /// The glyph ids of `text` in `face` with their pen positions, and the total advance.
    fn shape(&mut self, face: &Face, text: &str) -> (Vec<(u16, f32)>, f32) {
        let font = face.font();
        let mut chars = text.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            let glyph = font.charmap().map(c);
            let advance = font.glyph_metrics(&[]).scale(self.px).advance_width(glyph);
            return (vec![(glyph, 0.0)], advance);
        }
        let mut shaper = self
            .shaper
            .builder(font)
            .size(self.px)
            .variations(face.variations.iter())
            .build();
        shaper.add_str(text);
        let mut placed = Vec::new();
        let mut x = 0.0;
        shaper.shape_with(|cluster| {
            for g in cluster.glyphs {
                placed.push((g.id, x + g.x));
                x += g.advance;
            }
        });
        (placed, x)
    }

    /// Draws `key` at the cache's scale; none when it has nothing to draw.
    pub(super) fn draw(&mut self, key: GlyphKey) -> Option<Placed> {
        let m = self.metrics;
        let kind = key.kind();
        let placed = |at: (i32, i32), width: u32, height: u32, data: Vec<u8>| Placed {
            at,
            image: GlyphImage {
                width,
                height,
                kind,
                data,
            },
        };
        let drawn = match key {
            GlyphKey::Mask { face, glyph, synth } => {
                let face = self.fonts.face(face).clone();
                let img = outline(&mut self.scaler, &face, glyph, self.px, synth)?;
                let at = (img.left, m.baseline.0 as i32 - img.top);
                placed(at, img.width, img.height, img.data)
            }
            GlyphKey::Color {
                face,
                glyph,
                size: (w, h),
            } => {
                let face = self.fonts.face(face).clone();
                let img = match face.color {
                    ColorFormat::Colr => colr(&face, glyph, self.px, w, h),
                    ColorFormat::Bitmap => self
                        .bitmaps
                        .glyph(face.font(), glyph, self.px)
                        .map(|b| fit_bitmap(&b, w, h)),
                    ColorFormat::None => None,
                }?;
                placed((0, 0), img.width, img.height, img.rgba)
            }
            GlyphKey::Special(Special::Box(c)) => {
                let data = boxdraw::draw(c, m.width(), m.height(), m.thickness.0)?;
                placed((0, 0), m.width(), m.height(), data)
            }
            GlyphKey::Special(Special::Decor(d)) => {
                let mask = decoration_mask(d, &m);
                placed((0, mask.top as i32), mask.width, mask.height, mask.data)
            }
        };
        let texels = (drawn.image.width * drawn.image.height * kind.bytes_per_texel()) as usize;
        let inked = drawn.image.width > 0 && drawn.image.height > 0;
        (inked && drawn.image.data.len() >= texels).then_some(drawn)
    }
}
