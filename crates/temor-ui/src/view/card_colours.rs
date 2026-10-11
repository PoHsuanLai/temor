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
    /// Behind the selected block.
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
            selected_fill: Tint {
                color: palette.foreground,
                alpha: percent(8),
            },
        }
    }
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
            (c.selected_fill, palette.foreground, 20),
        ] {
            assert_eq!((tint.color, tint.alpha), (color, Alpha(alpha)));
        }
    }
}
