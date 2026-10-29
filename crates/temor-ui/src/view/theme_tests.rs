//! The palette a root resolves to: its ground reads as its scheme, and the ANSI slots are the
//! colours their names say, in both schemes.

use ds::prelude::{Accent, Appearance, Scheme, SpaceLook, SystemPrefs, Theme};
use temor_panes::vt::Rgb;

use ds_settings::Environment;

use super::{dark_by_default, palette_of, resolved_palette};
use crate::edge::{ThemeChoice, theme_choice};

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

#[test]
fn the_space_look_theme_wins_over_the_desktop_scheme() {
    let look = SpaceLook {
        theme: Theme::Dark,
        ..SpaceLook::default()
    };
    let light_desktop = SystemPrefs::default().with_scheme(Scheme::Light);
    let palette = resolved_palette(Appearance::default(), &look, light_desktop);
    assert_eq!(palette, palette_of(Scheme::Dark, Accent::default()));
}

#[test]
fn the_window_is_dark_unless_the_file_names_a_theme() {
    let light_desktop = SystemPrefs::default().with_scheme(Scheme::Light);
    let dark_desktop = SystemPrefs::default().with_scheme(Scheme::Dark);
    // (file, desktop, scheme the window resolves to)
    let cases = [
        (None, light_desktop, Scheme::Dark),
        (Some(""), light_desktop, Scheme::Dark),
        (
            Some("[appearance]\naccent = \"blue\"\n"),
            light_desktop,
            Scheme::Dark,
        ),
        (
            Some("[appearance]\ntheme = \"sepia\"\n"),
            light_desktop,
            Scheme::Dark,
        ),
        (Some("[appearance\n"), light_desktop, Scheme::Dark),
        (
            Some("[appearance]\ntheme = \"light\"\n"),
            dark_desktop,
            Scheme::Light,
        ),
        (
            Some("[appearance]\ntheme = \"dark\"\n"),
            light_desktop,
            Scheme::Dark,
        ),
        (
            Some("[appearance]\ntheme = \"system\"\n"),
            light_desktop,
            Scheme::Light,
        ),
        (
            Some("[appearance]\ntheme = \"system\"\n"),
            dark_desktop,
            Scheme::Dark,
        ),
    ];
    for (file, desktop, want) in cases {
        let mut env = Environment::default();
        env.settings.appearance.theme = match file.map(|f| f.contains("\"light\"")) {
            Some(true) => Theme::Light,
            Some(false) if file.is_some_and(|f| f.contains("\"dark\"")) => Theme::Dark,
            _ => Theme::System,
        };
        let env = dark_by_default(env, theme_choice(file));
        let got =
            ds::prelude::resolve(env.settings.appearance.appearance(), Theme::System, desktop);
        assert_eq!(got.scheme, want, "{file:?} on {desktop:?}");
    }
    assert_eq!(
        theme_choice(Some("[appearance]\ntheme = \"Light\"\n")),
        ThemeChoice::Named
    );
}
