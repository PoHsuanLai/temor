//! OS-specific edges. This is the only module allowed to name a target OS: everything else
//! reaches the platform through the small traits here.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod other;

#[cfg(target_os = "linux")]
pub(crate) use linux::SystemBitmapEmoji;
#[cfg(not(target_os = "linux"))]
pub(crate) use other::SystemBitmapEmoji;

/// A colour glyph as straight (not premultiplied) RGBA rows, at its own size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColorBitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Where bitmap colour emoji (CBDT on Linux, Apple Color Emoji's sbix on macOS) are drawn
/// from. COLRv1 does not come through here: it is painted portably through vello_cpu.
pub(crate) trait BitmapEmojiSource {
    /// Glyph `glyph` of `font` at `px` device pixels, or `None` when the face has no bitmap
    /// for it (or this platform has no source yet).
    fn glyph(&mut self, font: swash::FontRef<'_>, glyph: u16, px: f32) -> Option<ColorBitmap>;
}
