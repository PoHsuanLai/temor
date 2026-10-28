//! Building a cluster's glyphs: choose the face, shape, rasterise, pack and position.

use super::boxdraw;
use super::cache::{GlyphCache, GlyphKind, RasterKey, RunGlyph, Special};
use super::config::{FaceStyle, Slant, Weight};
use super::decor::decoration_mask;
use super::face::{ColorFormat, Face, FaceId, FaceRole, Synth};
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
            return self.special(Special::Box(c)).into_iter().collect();
        }
        let id = self.fonts.face_for(text, style);
        let face = self.fonts.face(id).clone();
        let (placed, advance) = self.shape(&face, text);
        let mut out = Vec::with_capacity(placed.len());
        if face.color != ColorFormat::None {
            let each = box_w / placed.len().max(1) as u32;
            let mut x = 0;
            for &(glyph, _) in &placed {
                if let Some(g) = self.color_glyph(id, &face, glyph, each, box_h) {
                    out.push(RunGlyph { dx: x, ..g });
                }
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
            if let Some(g) = self.mask_glyph(id, &face, glyph, synth) {
                out.push(RunGlyph {
                    dx: g.dx + x.round() as i32 + shift,
                    ..g
                });
            }
        }
        out
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

    fn mask_glyph(
        &mut self,
        face_id: FaceId,
        face: &Face,
        glyph: u16,
        synth: Synth,
    ) -> Option<RunGlyph> {
        let key = RasterKey {
            face: face_id,
            glyph,
            synth,
            color_box: (0, 0),
        };
        if let Some(known) = self.rasters.get(&key) {
            return *known;
        }
        let baseline = self.metrics.baseline.0 as i32;
        let placed = outline(&mut self.scaler, face, glyph, self.px, synth).and_then(|img| {
            let slot = self.store(GlyphKind::Mask, img.width, img.height, &img.data)?;
            Some(RunGlyph {
                dx: img.left,
                dy: baseline - img.top,
                slot,
                kind: GlyphKind::Mask,
            })
        });
        self.rasters.insert(key, placed);
        placed
    }

    fn color_glyph(
        &mut self,
        face_id: FaceId,
        face: &Face,
        glyph: u16,
        w: u32,
        h: u32,
    ) -> Option<RunGlyph> {
        let key = RasterKey {
            face: face_id,
            glyph,
            synth: Synth::default(),
            color_box: (w, h),
        };
        if let Some(known) = self.rasters.get(&key) {
            return *known;
        }
        let image = match face.color {
            ColorFormat::Colr => colr(face, glyph, self.px, w, h),
            ColorFormat::Bitmap => self
                .bitmaps
                .glyph(face.font(), glyph, self.px)
                .map(|b| fit_bitmap(&b, w, h)),
            ColorFormat::None => None,
        };
        let placed = image.and_then(|img| {
            let slot = self.store(GlyphKind::Color, img.width, img.height, &img.rgba)?;
            Some(RunGlyph {
                dx: 0,
                dy: 0,
                slot,
                kind: GlyphKind::Color,
            })
        });
        self.rasters.insert(key, placed);
        placed
    }

    pub(super) fn special(&mut self, special: Special) -> Option<RunGlyph> {
        if let Some(known) = self.specials.get(&special) {
            return *known;
        }
        let m = self.metrics;
        let (w, h, top, data) = match special {
            Special::Box(c) => {
                let data = boxdraw::draw(c, m.width(), m.height(), m.thickness.0)?;
                (m.width(), m.height(), 0, data)
            }
            Special::Decor(d) => {
                let mask = decoration_mask(d, &m);
                (mask.width, mask.height, mask.top, mask.data)
            }
        };
        let placed = self
            .store(GlyphKind::Mask, w, h, &data)
            .map(|slot| RunGlyph {
                dx: 0,
                dy: top as i32,
                slot,
                kind: GlyphKind::Mask,
            });
        self.specials.insert(special, placed);
        placed
    }
}
