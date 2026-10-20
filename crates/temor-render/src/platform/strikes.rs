//! Linux (CBDT) and macOS (sbix, Apple Color Emoji): swash reads the bitmap strikes straight from
//! the font file.

use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::Format;

use super::{BitmapEmojiSource, ColorBitmap};

/// Bitmap colour glyphs through swash.
pub(crate) struct SystemBitmapEmoji {
    scale: ScaleContext,
}

impl SystemBitmapEmoji {
    pub(crate) fn new() -> Self {
        Self {
            scale: ScaleContext::new(),
        }
    }
}

impl std::fmt::Debug for SystemBitmapEmoji {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemBitmapEmoji").finish_non_exhaustive()
    }
}

impl BitmapEmojiSource for SystemBitmapEmoji {
    fn glyph(&mut self, font: swash::FontRef<'_>, glyph: u16, px: f32) -> Option<ColorBitmap> {
        let mut scaler = self.scale.builder(font).size(px).build();
        let image = Render::new(&[Source::ColorBitmap(StrikeWith::BestFit)])
            .format(Format::Alpha)
            .render(&mut scaler, glyph)?;
        let (width, height) = (image.placement.width, image.placement.height);
        (image.content == Content::Color && width > 0 && height > 0).then_some(ColorBitmap {
            width,
            height,
            rgba: image.data,
        })
    }
}
