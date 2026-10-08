//! Masks for the underline styles that are not plain bars: curly, dotted and dashed. Each is
//! one cell wide and repeats across a run of cells.

use ds_core::word::Word;

use super::boxdraw::Canvas;
use crate::metrics::CellMetrics;

/// An underline drawn from a mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub(crate) enum Decoration {
    Curly,
    Dotted,
    Dashed,
}

/// A one-cell mask and how far below the cell's top it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecorMask {
    pub top: u32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// The mask for `decoration` in a cell of `m`.
pub(crate) fn decoration_mask(decoration: Decoration, m: &CellMetrics) -> DecorMask {
    let (w, ch, t) = (m.width(), m.height(), m.thickness.0.max(1));
    match decoration {
        Decoration::Curly => {
            // One full wave per cell, so neighbouring cells join.
            let amp = t.max(ch / 16).max(1) as f32;
            let height = (2.0 * amp) as u32 + t + 2;
            let top = m
                .underline
                .0
                .saturating_sub(amp as u32)
                .min(ch.saturating_sub(height));
            let height = height.min(ch - top);
            let mut c = Canvas::new(w, height);
            let mid = amp + t as f32 / 2.0 + 1.0;
            let half = t as f32 / 2.0 + 0.25;
            let period = w as f32;
            c.shape(|x, y| {
                let yc = mid - amp * (x / period * std::f32::consts::TAU).sin();
                (y - yc).abs() <= half
            });
            DecorMask {
                top,
                width: w,
                height,
                data: c.data,
            }
        }
        Decoration::Dotted | Decoration::Dashed => {
            let top = m.underline.0.min(ch - t);
            let mut c = Canvas::new(w, t);
            if decoration == Decoration::Dotted {
                let mut x = 0;
                while x < w {
                    c.rect(x as i32, 0, (x + t) as i32, t as i32);
                    x += 2 * t;
                }
            } else {
                let gap = (w / 6).max(1);
                c.rect(gap as i32, 0, (w - gap) as i32, t as i32);
            }
            DecorMask {
                top,
                width: w,
                height: t,
                data: c.data,
            }
        }
    }
}
