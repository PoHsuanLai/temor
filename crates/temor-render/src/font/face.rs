//! One loaded font face: its bytes, swash's handle on them, and what has to be synthesised.

use fontique::Blob;
use swash::{CacheKey, FontRef, tag_from_bytes};

/// A face's index in its `FontSet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FaceId(pub u16);

/// Whether glyphs are thickened because the family has no bold cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Embolden {
    No,
    Yes,
}

/// Whether glyphs are slanted because the family has no italic cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Skew {
    No,
    Yes,
}

/// How a face draws colour glyphs, if it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ColorFormat {
    /// Outlines only.
    None,
    /// COLR layers (v0 or v1): painted through vello_cpu, which swash cannot do for v1.
    Colr,
    /// Embedded bitmaps (CBDT, sbix): swash scales them.
    Bitmap,
}

/// Whether the face is one of the chosen family's cuts or a fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FaceRole {
    Primary,
    Fallback,
}

/// A face ready for swash and vello.
#[derive(Debug, Clone)]
pub(crate) struct Face {
    blob: Blob<u8>,
    index: u32,
    offset: u32,
    key: CacheKey,
    pub embolden: Embolden,
    pub skew: Skew,
    pub color: ColorFormat,
    pub role: FaceRole,
    /// Variation-axis settings (tag, value) for a variable family's bold or italic.
    pub variations: Vec<(u32, f32)>,
}

impl Face {
    /// Face `index` of `blob`, or `None` when swash cannot read it.
    pub fn load(blob: Blob<u8>, index: u32, role: FaceRole) -> Option<Self> {
        let font = FontRef::from_index(blob.as_ref(), index as usize)?;
        let (offset, key) = (font.offset, font.key);
        let has = |tag: &[u8; 4]| font.table(tag_from_bytes(tag)).is_some();
        let color = if has(b"COLR") && has(b"CPAL") {
            ColorFormat::Colr
        } else if has(b"CBDT") || has(b"sbix") {
            ColorFormat::Bitmap
        } else {
            ColorFormat::None
        };
        Some(Self {
            blob,
            index,
            offset,
            key,
            embolden: Embolden::No,
            skew: Skew::No,
            color,
            role,
            variations: Vec::new(),
        })
    }

    pub fn font(&self) -> FontRef<'_> {
        FontRef {
            data: self.blob.as_ref(),
            offset: self.offset,
            key: self.key,
        }
    }

    /// Whether the face has a glyph for `c`.
    pub fn maps(&self, c: char) -> bool {
        self.font().charmap().map(c) != 0
    }

    /// The face as vello_cpu (glifo, skrifa) takes it.
    pub fn font_data(&self) -> vello_cpu::peniko::FontData {
        vello_cpu::peniko::FontData::new(self.blob.clone(), self.index)
    }
}
