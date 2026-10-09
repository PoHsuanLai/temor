//! The palette a root resolves to: its ground reads as its scheme, and the ANSI slots are the
//! colours their names say, in both schemes.

use ds::prelude::{Accent, Scheme};
use temor_vt::Rgb;

use super::palette_of;

fn luma(c: Rgb) -> u32 {
    u32::from(c.r) * 3 + u32::from(c.g) * 6 + u32::from(c.b)
}

#[test]
fn the_ground_is_dark_in_a_dark_scheme_and_light_in_a_light_one() {
    let dark = palette_of(Scheme::Dark, Accent::default());
    let light = palette_of(Scheme::Light, Accent::default());
    assert!(luma(dark.background) < luma(dark.foreground));
    assert!(luma(light.background) > luma(light.foreground));
}

#[test]
fn ansi_red_is_red_and_the_selection_differs_from_the_ground_in_both_schemes() {
    for scheme in [Scheme::Light, Scheme::Dark] {
        let palette = palette_of(scheme, Accent::default());
        let red = palette.ansi[1];
        assert!(red.r > red.g && red.r > red.b, "{scheme:?}: {red:?}");
        assert_ne!(palette.selection, palette.background, "{scheme:?}");
    }
}
