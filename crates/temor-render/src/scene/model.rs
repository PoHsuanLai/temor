//! What a row turns into before either renderer draws it: positioned quads in two layers.

use temor_vt::Rgb;

use crate::font::GlyphKey;

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
    /// A glyph's coverage, times `color`.
    Mask {
        glyph: GlyphKey,
        color: Premul,
    },
    /// A colour glyph's premultiplied texels.
    Color {
        glyph: GlyphKey,
    },
}

impl QuadPaint {
    /// The glyph it draws, if any.
    #[cfg(feature = "gpu")]
    pub fn glyph(&self) -> Option<GlyphKey> {
        match *self {
            Self::Solid(_) => None,
            Self::Mask { glyph, .. } | Self::Color { glyph } => Some(glyph),
        }
    }
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

    /// The glyphs its quads draw.
    #[cfg(feature = "gpu")]
    pub fn glyphs(&self) -> impl Iterator<Item = GlyphKey> + '_ {
        self.bg
            .iter()
            .chain(&self.fg)
            .filter_map(|q| q.paint.glyph())
    }
}
