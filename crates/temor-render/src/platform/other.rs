//! Every OS but Linux: no bitmap emoji source yet, so COLR or monochrome glyphs are used.

use super::{BitmapEmojiSource, ColorBitmap};

/// No bitmap colour glyphs on this platform yet.
#[derive(Debug)]
pub(crate) struct SystemBitmapEmoji;

impl SystemBitmapEmoji {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl BitmapEmojiSource for SystemBitmapEmoji {
    // TODO(platform:macos): draw Apple Color Emoji (sbix) through Core Text, or check that
    // swash's sbix strikes are good enough there.
    // TODO(platform:windows): Segoe UI Emoji is COLR (handled portably); add a bitmap source
    // only if a CBDT font turns up.
    fn glyph(&mut self, _font: swash::FontRef<'_>, _glyph: u16, _px: f32) -> Option<ColorBitmap> {
        None
    }
}
