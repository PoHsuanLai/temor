//! The colours of the command cards, derived from the grid's palette so they follow the theme:
//! a rule is the foreground at 10% over the background, the failed fill red (ANSI 1) at 10%, the
//! stripe red at full strength, the selected fill the accent at 12%, and the line being typed
//! the foreground at 5%.

use temor_render::{Alpha, Palette, Tint};
use temor_vt::Rgb;

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
    /// Behind the line being typed at the prompt.
    pub input: Tint,
}

/// `percent` of full opacity.
fn percent(percent: u32) -> Alpha {
    Alpha(((percent * 255 + 50) / 100) as u8)
}

impl CardColours {
    /// The tints for `palette`; `accent` is the theme's accent colour (the palette's cursor
    /// colour follows it).
    pub fn of(palette: &Palette, accent: Rgb) -> Self {
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
                color: accent,
                alpha: percent(12),
            },
            input: Tint {
                color: palette.foreground,
                alpha: percent(5),
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
        let accent = Rgb::new(10, 20, 30);
        let c = CardColours::of(&palette, accent);
        // (tint, colour, alpha)
        for (tint, color, alpha) in [
            (c.rule, palette.foreground, 26),
            (c.failed_fill, palette.ansi[1], 26),
            (c.failed_stripe, palette.ansi[1], 255),
            (c.selected_fill, accent, 31),
            (c.input, palette.foreground, 13),
        ] {
            assert_eq!((tint.color, tint.alpha), (color, Alpha(alpha)));
        }
    }
}
