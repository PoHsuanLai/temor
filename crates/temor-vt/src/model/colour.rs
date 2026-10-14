//! Colours as the terminal resolves them.

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

/// A colour as the program named it, before the palette turned it into an [`Rgb`]. Kitty's
/// unicode placeholders carry an image id in the foreground colour, which only the name keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorRef {
    /// No colour was set.
    #[default]
    Unset,
    /// A palette entry.
    Palette(u8),
    Rgb(Rgb),
}

impl ColorRef {
    /// The 24-bit number kitty reads from the colour: the palette index, or the channels packed
    /// as `0xRRGGBB`; 0 when unset.
    pub const fn kitty_id(self) -> u32 {
        match self {
            Self::Unset => 0,
            Self::Palette(i) => i as u32,
            Self::Rgb(c) => ((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32,
        }
    }
}

/// Whether the person's appearance is light or dark; programs may ask (CSI ? 996 n).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorScheme {
    Light,
    #[default]
    Dark,
}
