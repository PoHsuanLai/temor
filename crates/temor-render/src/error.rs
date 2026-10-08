//! The crate's one error type.

/// What can stop a frame from being captured or drawn.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// No font could be loaded for the configured family or any fallback. Pick another family,
    /// or install a monospace font.
    #[error("no usable font for “{family}”")]
    FontNotFound { family: String },
    /// A bundled font's bytes are not a font swash can read.
    #[error("font data number {index} is not a readable font")]
    BadFontData { index: usize },
    /// The pane is larger than a render target may be (65,535 pixels a side for the CPU path,
    /// the device limit for the GPU path). Shrink the pane.
    #[error("pane of {width}x{height} pixels is too large to render")]
    PaneTooLarge { width: u32, height: u32 },
    /// The terminal core failed while handing over a frame; drop the frame and capture again.
    #[error("terminal frame: {0}")]
    Vt(#[from] temor_vt::VtError),
}
