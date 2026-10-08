//! What a row turns into before either renderer draws it: positioned quads in two layers.

use temor_vt::Rgb;

use crate::font::Slot;

/// A premultiplied RGBA8 colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Premul(pub [u8; 4]);

impl Premul {
    pub fn opaque(c: Rgb) -> Self {
        Self([c.r, c.g, c.b, 255])
    }
}

/// How a quad is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum QuadPaint {
    Solid(Premul),
    /// Coverage from the alpha atlas, times `color`.
    Mask {
        slot: Slot,
        color: Premul,
    },
    /// Premultiplied texels from the colour atlas.
    Color {
        slot: Slot,
    },
}

/// A rectangle in device pixels from the pane's top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Quad {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub paint: QuadPaint,
}

impl Quad {
    pub fn solid(x: i32, y: i32, w: u32, h: u32, color: Rgb) -> Self {
        Self {
            x,
            y,
            w,
            h,
            paint: QuadPaint::Solid(Premul::opaque(color)),
        }
    }
}

/// One row's quads: backgrounds (and a block cursor) under any `BelowText` images, then text,
/// decorations and other cursors over them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RowScene {
    pub bg: Vec<Quad>,
    pub fg: Vec<Quad>,
}

impl RowScene {
    pub fn clear(&mut self) {
        self.bg.clear();
        self.fg.clear();
    }
}
