//! Colours as the terminal resolves them.

use ds_core::word::Word;

/// A 24-bit colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// The colour with these channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Whether the person's appearance is light or dark; programs may ask (CSI ? 996 n).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum ColorScheme {
    Light,
    #[default]
    Dark,
}
