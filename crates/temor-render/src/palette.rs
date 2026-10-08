//! The grid's colours as the UI hands them in. The UI maps quire's terminal palette tokens onto
//! this plain struct (v0.2.31), so light/dark and the Space theme apply.

use temor_vt::Rgb;

/// The colours the renderer draws with.
///
/// libghostty resolves every cell colour to RGB with its own palette. Until temor-vt can set the
/// terminal's default colours (PLAN §5, upstream ask), the renderer maps them here: a cell
/// colour equal to one of the terminal's 16 ANSI entries draws as the matching entry of `ansi`,
/// and the default foreground and background draw as `foreground` and `background`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Palette {
    /// Black, red, green, yellow, blue, magenta, cyan, white, then their bright forms.
    pub ansi: [Rgb; 16],
    pub foreground: Rgb,
    pub background: Rgb,
    /// The cursor, unless the program set its own (OSC 12).
    pub cursor: Rgb,
    /// The background of selected cells.
    pub selection: Rgb,
}

impl Default for Palette {
    /// A neutral dark palette, for tests and until the UI passes quire's tokens.
    fn default() -> Self {
        const fn c(hex: u32) -> Rgb {
            Rgb::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
        }
        Self {
            ansi: [
                c(0x1d1f21),
                c(0xcc6666),
                c(0xb5bd68),
                c(0xf0c674),
                c(0x81a2be),
                c(0xb294bb),
                c(0x8abeb7),
                c(0xc5c8c6),
                c(0x666666),
                c(0xd54e53),
                c(0xb9ca4a),
                c(0xe7c547),
                c(0x7aa6da),
                c(0xc397d8),
                c(0x70c0b1),
                c(0xeaeaea),
            ],
            foreground: c(0xd8d8d8),
            background: c(0x181818),
            cursor: c(0xe8e8e8),
            selection: c(0x3a4a5c),
        }
    }
}
