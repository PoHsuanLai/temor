//! The window's colours on a session's screen: programs that ask for a colour (OSC 4, 10, 11, 12)
//! get the window's answer.

use temor_vt::{Rgb, Terminal};
use temor_wire::message::{Palette, Rgb as WireRgb};

use crate::error::Error;

fn rgb(colour: WireRgb) -> Rgb {
    Rgb::new(colour.r, colour.g, colour.b)
}

/// The 256 colours the window's 16 imply: the 16 as given, then xterm's 6 by 6 by 6 cube and its
/// 24 greys, which no theme changes.
pub(super) fn full_palette(ansi: &[WireRgb; 16]) -> [Rgb; 256] {
    let mut all = [Rgb::new(0, 0, 0); 256];
    for (slot, colour) in all.iter_mut().zip(ansi) {
        *slot = rgb(*colour);
    }
    for (index, slot) in all.iter_mut().enumerate().skip(16) {
        *slot = if index < 232 {
            let n = index - 16;
            Rgb::new(cube(n / 36), cube(n / 6 % 6), cube(n % 6))
        } else {
            let grey = u8::try_from(8 + 10 * (index - 232)).unwrap_or(u8::MAX);
            Rgb::new(grey, grey, grey)
        };
    }
    all
}

/// One channel of the cube: 0, then 95 and 40 more for each step.
fn cube(step: usize) -> u8 {
    match step {
        0 => 0,
        n => u8::try_from(55 + 40 * n).unwrap_or(u8::MAX),
    }
}

/// Sets the screen's default colours to `palette`'s.
pub(super) fn apply_palette(screen: &mut Terminal, palette: &Palette) -> Result<(), Error> {
    screen.set_default_palette(Some(&full_palette(&palette.ansi)))?;
    screen.set_default_foreground(Some(rgb(palette.foreground)))?;
    screen.set_default_background(Some(rgb(palette.background)))?;
    screen.set_default_cursor(Some(rgb(palette.cursor)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sixteen_are_the_windows_and_the_rest_are_xterms() {
        let mut ansi = [WireRgb { r: 1, g: 2, b: 3 }; 16];
        ansi[15] = WireRgb { r: 9, g: 8, b: 7 };
        let all = full_palette(&ansi);
        let cases = [
            (0, Rgb::new(1, 2, 3)),
            (15, Rgb::new(9, 8, 7)),
            (16, Rgb::new(0, 0, 0)),
            (17, Rgb::new(0, 0, 95)),
            (196, Rgb::new(255, 0, 0)),
            (231, Rgb::new(255, 255, 255)),
            (232, Rgb::new(8, 8, 8)),
            (255, Rgb::new(238, 238, 238)),
        ];
        for (index, want) in cases {
            assert_eq!(all[index], want, "colour {index}");
        }
    }
}
