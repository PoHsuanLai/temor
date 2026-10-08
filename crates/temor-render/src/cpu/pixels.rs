//! Pixel operations on a premultiplied RGBA8 buffer, clipped to a horizontal band.

use crate::font::Atlas;
use crate::frame::ImagePlacement;
use crate::scene::{Premul, Quad, QuadPaint};

/// A premultiplied RGBA8 buffer, writable only between rows `band.0` and `band.1`.
pub(crate) struct Target<'a> {
    pub data: &'a mut [u8],
    pub width: u32,
    pub height: u32,
    pub band: (i32, i32),
}

/// Premultiplied source-over.
#[inline]
fn over(dst: &mut [u8], src: [u8; 4]) {
    let ia = 255 - u32::from(src[3]);
    for c in 0..4 {
        dst[c] = (u32::from(src[c]) + (u32::from(dst[c]) * ia + 127) / 255) as u8;
    }
}

#[inline]
fn scale(c: [u8; 4], a: u32) -> [u8; 4] {
    let m = |v: u8| ((u32::from(v) * a + 127) / 255) as u8;
    [m(c[0]), m(c[1]), m(c[2]), m(c[3])]
}

impl Target<'_> {
    /// The visible part of `x..x+w`, `y..y+h`: (x0, y0, x1, y1).
    fn clip(&self, x: i32, y: i32, w: u32, h: u32) -> Option<(i32, i32, i32, i32)> {
        let x0 = x.max(0);
        let y0 = y.max(self.band.0).max(0);
        let x1 = (x + w as i32).min(self.width as i32);
        let y1 = (y + h as i32).min(self.band.1).min(self.height as i32);
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
    }

    fn at(&mut self, x: i32, y: i32) -> &mut [u8] {
        let i = ((y as u32 * self.width + x as u32) * 4) as usize;
        &mut self.data[i..i + 4]
    }

    /// Fills the whole band with an opaque colour.
    pub fn clear_band(&mut self, color: Premul) {
        let Some((x0, y0, x1, y1)) = self.clip(
            0,
            self.band.0,
            self.width,
            (self.band.1 - self.band.0) as u32,
        ) else {
            return;
        };
        for y in y0..y1 {
            for x in x0..x1 {
                self.at(x, y).copy_from_slice(&color.0);
            }
        }
    }

    pub fn quad(&mut self, q: &Quad, alpha: &Atlas, color: &Atlas) {
        let Some((x0, y0, x1, y1)) = self.clip(q.x, q.y, q.w, q.h) else {
            return;
        };
        match q.paint {
            QuadPaint::Solid(c) => {
                for y in y0..y1 {
                    for x in x0..x1 {
                        if c.0[3] == 255 {
                            self.at(x, y).copy_from_slice(&c.0);
                        } else {
                            over(self.at(x, y), c.0);
                        }
                    }
                }
            }
            QuadPaint::Mask { slot, color: c } => {
                for y in y0..y1 {
                    let src = alpha.row(slot, (y - q.y) as u32);
                    for x in x0..x1 {
                        let cov = u32::from(src[(x - q.x) as usize]);
                        if cov != 0 {
                            over(self.at(x, y), scale(c.0, cov));
                        }
                    }
                }
            }
            QuadPaint::Color { slot } => {
                for y in y0..y1 {
                    let src = color.row(slot, (y - q.y) as u32);
                    for x in x0..x1 {
                        let i = ((x - q.x) * 4) as usize;
                        let p = [src[i], src[i + 1], src[i + 2], src[i + 3]];
                        if p[3] != 0 {
                            over(self.at(x, y), p);
                        }
                    }
                }
            }
        }
    }

    /// Draws a kitty placement, nearest-neighbour scaled from its source rectangle, with its
    /// top-left at `(x, y)`.
    pub fn image(&mut self, p: &ImagePlacement, x: i32, y: i32) {
        let img = &p.image;
        let [sx, sy, sw, sh] = p.source;
        let (dw, dh) = (p.width.0, p.height.0);
        if dw == 0 || dh == 0 || sw == 0 || sh == 0 {
            return;
        }
        let Some((x0, y0, x1, y1)) = self.clip(x, y, dw, dh) else {
            return;
        };
        for py in y0..y1 {
            let ty = sy + ((py - y) as u64 * u64::from(sh) / u64::from(dh)) as u32;
            if ty >= img.height {
                continue;
            }
            for px in x0..x1 {
                let tx = sx + ((px - x) as u64 * u64::from(sw) / u64::from(dw)) as u32;
                if tx >= img.width {
                    continue;
                }
                let i = ((ty * img.width + tx) * 4) as usize;
                let Some(s) = img.rgba.get(i..i + 4) else {
                    continue;
                };
                over(self.at(px, py), [s[0], s[1], s[2], s[3]]);
            }
        }
    }
}
