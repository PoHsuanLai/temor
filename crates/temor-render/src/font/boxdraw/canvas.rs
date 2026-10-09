//! A one-cell coverage mask that box-drawing characters are drawn into.

/// Samples per pixel side when filling shapes with soft edges.
const SS: u32 = 4;

/// A `w` x `h` coverage mask (0 empty, 255 full).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canvas {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            data: vec![0; (w * h) as usize],
        }
    }

    /// Fills `[x0, x1) x [y0, y1)`, clipped to the canvas.
    pub fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.rect_alpha(x0, y0, x1, y1, 255);
    }

    /// Fills a rectangle with coverage `a`, keeping the larger coverage where filled already.
    pub fn rect_alpha(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, a: u8) {
        let (w, h) = (self.w as i32, self.h as i32);
        for y in y0.max(0)..y1.min(h) {
            for x in x0.max(0)..x1.min(w) {
                let p = &mut self.data[(y * w + x) as usize];
                *p = (*p).max(a);
            }
        }
    }

    /// Fills where `inside(x, y)` holds (pixel coordinates, centres at +0.5), antialiased by
    /// supersampling.
    pub fn shape(&mut self, inside: impl Fn(f32, f32) -> bool) {
        let step = 1.0 / SS as f32;
        for y in 0..self.h {
            for x in 0..self.w {
                let mut hits = 0u32;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let px = x as f32 + (sx as f32 + 0.5) * step;
                        let py = y as f32 + (sy as f32 + 0.5) * step;
                        hits += u32::from(inside(px, py));
                    }
                }
                let a = (hits * 255 / (SS * SS)) as u8;
                let p = &mut self.data[(y * self.w + x) as usize];
                *p = (*p).max(a);
            }
        }
    }

    /// The canvas mirrored over its diagonal (`w` and `h` swap).
    pub fn transposed(&self) -> Self {
        let mut out = Self::new(self.h, self.w);
        for y in 0..self.h {
            for x in 0..self.w {
                out.data[(x * self.h + y) as usize] = self.data[(y * self.w + x) as usize];
            }
        }
        out
    }

    /// Keeps the larger coverage of `self` and `other` (same size).
    pub fn merge(&mut self, other: &Self) {
        for (a, b) in self.data.iter_mut().zip(&other.data) {
            *a = (*a).max(*b);
        }
    }
}
