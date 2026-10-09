//! Kitty placements read under the terminal lock, with pixels premultiplied once per image
//! generation.

use std::collections::HashMap;
use std::sync::Arc;

use temor_vt::{
    ImageId, KittyImage, KittyLayer, KittyPixelFormat, KittyPixels, PlacementKind, Terminal,
    Visibility,
};

use super::model::{ImagePixels, ImagePlacement};

/// The layers in drawing order.
const LAYERS: [KittyLayer; 3] = [
    KittyLayer::BelowBackground,
    KittyLayer::BelowText,
    KittyLayer::AboveText,
];

/// Premultiplied copies of kitty images, keyed by id and replaced when the generation moves.
#[derive(Debug, Default)]
pub(crate) struct ImageCache {
    images: HashMap<ImageId, Arc<ImagePixels>>,
}

impl ImageCache {
    /// Every visible direct placement, by layer, in libghostty's order. Images no longer placed
    /// are dropped from the cache.
    pub fn collect(&mut self, term: &Terminal) -> Vec<ImagePlacement> {
        let Some(kitty) = term.kitty() else {
            self.images.clear();
            return Vec::new();
        };
        let mut out = Vec::new();
        for layer in LAYERS {
            let Ok(placements) = kitty.placements(layer) else {
                continue;
            };
            for p in placements {
                if p.kind == PlacementKind::Virtual || p.render.visibility == Visibility::Hidden {
                    continue;
                }
                let Some(image) = self.pixels(&p.image) else {
                    continue;
                };
                let r = p.render;
                out.push(ImagePlacement {
                    layer,
                    image,
                    col: r.viewport_col,
                    row: r.viewport_row,
                    offset: (r.x_offset, r.y_offset),
                    width: temor_vt::Px(r.pixel_width),
                    height: temor_vt::Px(r.pixel_height),
                    source: [r.source_x, r.source_y, r.source_width, r.source_height],
                });
            }
        }
        self.images
            .retain(|id, _| out.iter().any(|p| p.image.id == *id));
        out
    }

    fn pixels(&mut self, image: &KittyImage<'_>) -> Option<Arc<ImagePixels>> {
        if let Some(known) = self.images.get(&image.id)
            && known.generation == image.generation
        {
            return Some(known.clone());
        }
        let KittyPixels::Ready(data) = image.pixels else {
            return None;
        };
        let (w, h) = (image.width.0, image.height.0);
        let pixels = Arc::new(ImagePixels {
            id: image.id,
            generation: image.generation,
            width: w,
            height: h,
            rgba: premultiply(data, image.format, w, h),
        });
        self.images.insert(image.id, pixels.clone());
        Some(pixels)
    }
}

/// `data` in `format` as premultiplied RGBA8, padded or cut to `w` x `h`.
pub(crate) fn premultiply(data: &[u8], format: KittyPixelFormat, w: u32, h: u32) -> Vec<u8> {
    let n = (w as usize).saturating_mul(h as usize);
    let mul = |c: u8, a: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
    let mut out = Vec::with_capacity(n * 4);
    match format {
        KittyPixelFormat::Rgb => {
            for p in data.as_chunks::<3>().0.iter().take(n) {
                out.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        KittyPixelFormat::Gray => {
            for &v in data.iter().take(n) {
                out.extend_from_slice(&[v, v, v, 255]);
            }
        }
        KittyPixelFormat::GrayAlpha => {
            for p in data.as_chunks::<2>().0.iter().take(n) {
                let v = mul(p[0], p[1]);
                out.extend_from_slice(&[v, v, v, p[1]]);
            }
        }
        KittyPixelFormat::Rgba => {
            for p in data.as_chunks::<4>().0.iter().take(n) {
                out.extend_from_slice(&[mul(p[0], p[3]), mul(p[1], p[3]), mul(p[2], p[3]), p[3]]);
            }
        }
    }
    out.resize(n * 4, 0);
    out
}
