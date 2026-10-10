//! The window's colours, sent so programs on the host that ask for them (OSC 4, 10, 11, 12) get
//! the window's answer and not a default.

use serde::{Deserialize, Serialize};

/// A 24-bit colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

/// The colours a terminal answers queries with: the 16 ANSI colours (black to bright white, in
/// the usual order), the default text and background, and the cursor. The rest of the 256
/// colours follow from these on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Palette {
    /// Colours 0 to 15.
    pub ansi: [Rgb; 16],
    /// The default text colour.
    pub foreground: Rgb,
    /// The default background.
    pub background: Rgb,
    /// The cursor.
    pub cursor: Rgb,
}
