//! The colours of the command cards, derived from the grid's palette so they follow the theme:
//! the failed fill red (ANSI 1) at 10%, the stripe red at full strength, and the selected fill
//! the foreground at 8%: a neutral wash, as quire's source list selects while the keyboard is
//! elsewhere.

use temor_render::{Alpha, Palette, Tint};

/// The tints the card bands use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardColours {
    /// Behind a failed block.
    pub failed_fill: Tint,
    /// The bar at a failed block's left edge.
    pub failed_stripe: Tint,
    /// The bar at a bookmarked block's left edge.
    pub bookmark_stripe: Tint,
    /// Behind a picked block.
    pub selected_fill: Tint,
}

/// `percent` of full opacity.
fn percent(percent: u32) -> Alpha {
    Alpha(((percent * 255 + 50) / 100) as u8)
}

impl CardColours {
    /// The tints for `palette`.
    pub fn of(palette: &Palette) -> Self {
        let red = palette.ansi[1];
        Self {
            failed_fill: Tint {
                color: red,
                alpha: percent(10),
            },
            failed_stripe: Tint {
                color: red,
                alpha: Alpha(255),
            },
            // Warp's bookmark is yellow; the palette's bright yellow reads on light and dark.
            bookmark_stripe: Tint {
                color: palette.ansi[11],
                alpha: Alpha(255),
            },
            // A picked block reads as selected text does: the selection's colour, lighter.
            selected_fill: Tint {
                color: palette.selection,
                alpha: percent(45),
            },
        }
    }
}

/// Which fill lies behind a block's rows, as the bands paint it: a pick wins over a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardFill {
    None,
    Failed,
    Picked,
}

/// The colour a card is drawn on: the palette's background under the block's fill, as CSS. A
/// card with no fill is raised a little off the background (the foreground at 5%), so it reads
/// as a card on any theme.
pub fn card_ground(palette: &Palette, fill: CardFill) -> String {
    let c = card_ground_rgb(palette, fill);
    format!("rgb({},{},{})", c.r, c.g, c.b)
}

/// The colour a card is drawn on, as channels.
pub fn card_ground_rgb(palette: &Palette, fill: CardFill) -> temor_vt::Rgb {
    let colours = CardColours::of(palette);
    let tint = match fill {
        CardFill::None => Tint {
            color: palette.foreground,
            alpha: percent(5),
        },
        CardFill::Failed => colours.failed_fill,
        CardFill::Picked => colours.selected_fill,
    };
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
    fn the_tints_follow_the_palette_at_their_strengths() {
        let palette = Palette::default();
        let c = CardColours::of(&palette);
        // (tint, colour, alpha)
        for (tint, color, alpha) in [
            (c.failed_fill, palette.ansi[1], 26),
            (c.failed_stripe, palette.ansi[1], 255),
            (c.selected_fill, palette.selection, 115),
            (c.bookmark_stripe, palette.ansi[11], 255),
        ] {
            assert_eq!((tint.color, tint.alpha), (color, Alpha(alpha)));
        }
    }

    #[test]
    fn a_ground_is_the_background_under_the_fill() {
        let mut ansi = Palette::default().ansi;
        ansi[1] = temor_vt::Rgb { r: 255, g: 0, b: 0 };
        let palette = Palette {
            background: temor_vt::Rgb { r: 0, g: 0, b: 0 },
            ansi,
            ..Palette::default()
        };
        // White at 5% (13 of 255) over black.
        let palette = Palette {
            foreground: temor_vt::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            ..palette
        };
        assert_eq!(card_ground(&palette, CardFill::None), "rgb(13,13,13)");
        // Red at 10% (26 of 255) over black.
        assert_eq!(card_ground(&palette, CardFill::Failed), "rgb(26,0,0)");
    }
}
