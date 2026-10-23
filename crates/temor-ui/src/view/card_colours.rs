//! The colours a block's card is drawn on, derived from the grid's palette so they follow the
//! theme: a failed card on red (ANSI 1) at 10%, a picked one on the selection's colour at 45%,
//! and any other raised a little off the background (the foreground at 5%).

use temor_render::{Alpha, Palette, Tint};

/// `percent` of full opacity.
fn percent(percent: u32) -> Alpha {
    Alpha(((percent * 255 + 50) / 100) as u8)
}

/// Which fill lies behind a block's card: a pick wins over a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardFill {
    None,
    Failed,
    Picked,
}

/// The tint a card's fill lays over the palette's background.
fn tint_of(palette: &Palette, fill: CardFill) -> Tint {
    match fill {
        CardFill::None => Tint {
            color: palette.foreground,
            alpha: percent(5),
        },
        CardFill::Failed => Tint {
            color: palette.ansi[1],
            alpha: percent(10),
        },
        // A picked block reads as selected text does: the selection's colour, lighter.
        CardFill::Picked => Tint {
            color: palette.selection,
            alpha: percent(45),
        },
    }
}

/// The colour a card is drawn on: the palette's background under the block's fill.
pub fn card_ground_rgb(palette: &Palette, fill: CardFill) -> temor_vt::Rgb {
    let tint = tint_of(palette, fill);
    let a = u32::from(tint.alpha.0);
    let mix = |under: u8, over: u8| {
        ((u32::from(under) * (255 - a) + u32::from(over) * a + 127) / 255).min(255) as u8
    };
    let (bg, c) = (palette.background, tint.color);
    temor_vt::Rgb::new(mix(bg.r, c.r), mix(bg.g, c.g), mix(bg.b, c.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ground_is_the_background_under_the_fill() {
        let mut ansi = Palette::default().ansi;
        ansi[1] = temor_vt::Rgb { r: 255, g: 0, b: 0 };
        let palette = Palette {
            background: temor_vt::Rgb { r: 0, g: 0, b: 0 },
            foreground: temor_vt::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            selection: temor_vt::Rgb { r: 0, g: 0, b: 200 },
            ansi,
            ..Palette::default()
        };
        // (fill, want): white at 5% (13 of 255), red at 10% (26), the selection at 45% (115).
        for (fill, want) in [
            (CardFill::None, temor_vt::Rgb::new(13, 13, 13)),
            (CardFill::Failed, temor_vt::Rgb::new(26, 0, 0)),
            (CardFill::Picked, temor_vt::Rgb::new(0, 0, 90)),
        ] {
            assert_eq!(card_ground_rgb(&palette, fill), want, "{fill:?}");
        }
    }
}
