//! The colours of the command cards, derived from the grid's palette so they follow the theme:
//! a rule is the foreground at 10% over the background, the failed fill red (ANSI 1) at 10%, the
//! stripe red at full strength, and the selected fill the foreground at 8%: a neutral wash, as
//! quire's source list selects while the keyboard is elsewhere.

use temor_render::{Alpha, Palette, Tint};

/// The tints the card bands use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardColours {
    /// The line above every block.
    pub rule: Tint,
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
            rule: Tint {
                color: palette.foreground,
                alpha: percent(10),
            },
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

/// The colour a block's row shows where nothing else is drawn: the palette's background under
/// the block's fill, as CSS. `None` without a fill (the pane's own ground shows).
pub fn card_ground(palette: &Palette, fill: CardFill) -> Option<String> {
    let colours = CardColours::of(palette);
    let tint = match fill {
        CardFill::None => return None,
        CardFill::Failed => colours.failed_fill,
        CardFill::Picked => colours.selected_fill,
    };
    let a = u32::from(tint.alpha.0);
    let mix =
        |under: u8, over: u8| (u32::from(under) * (255 - a) + u32::from(over) * a + 127) / 255;
    let (bg, c) = (palette.background, tint.color);
    Some(format!(
        "rgb({},{},{})",
        mix(bg.r, c.r),
        mix(bg.g, c.g),
        mix(bg.b, c.b)
    ))
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
            (c.rule, palette.foreground, 26),
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
        assert_eq!(card_ground(&palette, CardFill::None), None);
        // Red at 10% (26 of 255) over black.
        assert_eq!(
            card_ground(&palette, CardFill::Failed).as_deref(),
            Some("rgb(26,0,0)")
        );
    }
}
