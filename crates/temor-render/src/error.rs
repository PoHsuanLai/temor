//! The crate's one error type.

/// What can stop a frame from being captured or drawn.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// No font could be loaded for the configured family or any fallback. Pick another family,
    /// or install a monospace font.
    #[error("no usable font for “{family}”")]
    FontNotFound { family: String },
    /// A bundled font's bytes are not a font swash can read.
    #[error("font data number {index} is not a readable font")]
    BadFontData { index: usize },
    /// The pane is larger than a render target may be (16,384 pixels a side for the CPU path,
    /// the device's texture limit for the GPU path). Shrink the pane.
    #[error("pane of {width}x{height} pixels is too large to render")]
    PaneTooLarge { width: u32, height: u32 },
    /// The glyphs of one row do not fit the glyph atlases on their own, even after they were
    /// built again: the atlas cap is too small for what is on screen. (A frame whose rows fit
    /// one at a time is drawn in groups.) Draw again next frame.
    #[error("the glyphs on screen do not fit the glyph atlas")]
    GlyphsDoNotFit,
    /// The GPU renderer has no render target for the pane it was asked to draw. This is an
    /// internal invariant (the target is created with the pane), reported rather than panicked.
    #[error("the pane has no render target")]
    NoRenderTarget,
    /// The terminal core failed while handing over a frame; drop the frame and capture again.
    #[error("terminal frame: {0}")]
    Vt(#[from] temor_vt::VtError),
}
