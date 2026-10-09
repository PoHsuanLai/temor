//! The grid's colours as the UI hands them in. The UI maps quire's terminal palette tokens onto
//! this plain struct (v0.2.31), so light/dark and the Space theme apply.

use temor_vt::Rgb;
#[cfg(feature = "ghostty")]
use temor_vt::Terminal;

#[cfg(feature = "ghostty")]
use crate::error::RenderError;

/// The grid's colours. [`Palette::apply`] makes them the terminal's defaults, so libghostty
/// resolves every cell with them and a program's own overrides (OSC 4, 10, 11, 12) still win.
/// The renderer itself only reads `selection` (and `cursor` when the terminal has none).
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

impl Palette {
    /// The full 256-colour palette: `ansi`, then xterm's 6x6x6 cube and grey ramp.
    pub fn table(&self) -> [Rgb; 256] {
        let mut out = [Rgb::default(); 256];
        out[..16].copy_from_slice(&self.ansi);
        let level = |v: usize| if v == 0 { 0 } else { (55 + v * 40) as u8 };
        for i in 0..216 {
            out[16 + i] = Rgb::new(level(i / 36), level(i / 6 % 6), level(i % 6));
        }
        for i in 0..24 {
            let v = (8 + i * 10) as u8;
            out[232 + i] = Rgb::new(v, v, v);
        }
        out
    }

    /// Makes these the terminal's default colours. Call it once per terminal and whenever the
    /// theme changes; the next captured frame is redrawn whole.
    #[cfg(feature = "ghostty")]
    pub fn apply(&self, term: &mut Terminal) -> Result<(), RenderError> {
        term.set_default_palette(Some(&self.table()))?;
        term.set_default_foreground(Some(self.foreground))?;
        term.set_default_background(Some(self.background))?;
        term.set_default_cursor(Some(self.cursor))?;
        Ok(())
    }
}
