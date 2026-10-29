//! The window's colours as the helper takes them (PLAN §9b): the grid's palette without the
//! selection colour, which only this window draws.

use temor_wire::message::{Palette, Rgb};

fn colour(from: temor_vt::Rgb) -> Rgb {
    Rgb {
        r: from.r,
        g: from.g,
        b: from.b,
    }
}

/// `palette` for the wire.
pub fn wire_palette(palette: &temor_render::Palette) -> Palette {
    Palette {
        ansi: palette.ansi.map(colour),
        foreground: colour(palette.foreground),
        background: colour(palette.background),
        cursor: colour(palette.cursor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_vt::Rgb as Vt;

    #[test]
    fn every_colour_the_helper_takes_comes_across_in_order() {
        let mut ours = temor_render::Palette::default();
        for (n, slot) in ours.ansi.iter_mut().enumerate() {
            *slot = Vt::new(n as u8, 1, 2);
        }
        ours.foreground = Vt::new(10, 11, 12);
        ours.background = Vt::new(20, 21, 22);
        ours.cursor = Vt::new(30, 31, 32);
        let wire = wire_palette(&ours);
        let rgb = |r, g, b| Rgb { r, g, b };
        assert_eq!(wire.ansi[0], rgb(0, 1, 2));
        assert_eq!(wire.ansi[15], rgb(15, 1, 2));
        assert_eq!(wire.foreground, rgb(10, 11, 12));
        assert_eq!(wire.background, rgb(20, 21, 22));
        assert_eq!(wire.cursor, rgb(30, 31, 32));
    }
}
