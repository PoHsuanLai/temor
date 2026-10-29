//! The grid's colours from quire's terminal palette tokens (PLAN §5): the resolved scheme and
//! accent of the root decide them, so light and dark, and the Space look that overrides the
//! theme, all apply to the terminal too.

use std::sync::Arc;

use dioxus::prelude::*;
use ds::base::spawner::Spawner;
use ds::prelude::{Accent, Appearance, Scheme, SpaceLook, SystemPrefs, Theme, resolve};
use ds::style::tokens::hex::{Colour, Hex};
use ds::style::tokens::terminal::TerminalColour;
use ds_settings::{
    AppName, AppearanceFile, ConfigRoot, Environment, Store, SystemPrefsSource, use_environment,
};
use temor_panes::render::Palette;
use temor_panes::vt::Rgb;

use crate::edge::{ThemeChoice, read_theme_choice};

/// Where the look comes from: the design system's own settings (`appearance.toml`, the person's
/// theme, accent and motion) and the desktop's preferences. The window follows both live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceSource {
    pub store: Store,
    pub system: SystemPrefsSource,
}

impl AppearanceSource {
    /// The person's: `$XDG_CONFIG_HOME/quire/appearance.toml` and the desktop's portal.
    pub fn desktop() -> Self {
        Self {
            store: Store::new(ConfigRoot::Xdg, AppName::QUIRE),
            system: SystemPrefsSource::Portal,
        }
    }

    /// A scratch config tree and fixed preferences, for tests and headless runs.
    pub fn scratch(config_dir: std::path::PathBuf, system: SystemPrefs) -> Self {
        Self {
            store: Store::new(ConfigRoot::Scratch(config_dir), AppName::QUIRE),
            system: SystemPrefsSource::Fixed(system),
        }
    }
}

/// `env` with the theme temor defaults to: dark, when the person has named none (`choice`) and
/// the file leaves it at "follow the desktop". An explicit `system`, `light` or `dark` wins.
pub fn dark_by_default(mut env: Environment, choice: ThemeChoice) -> Environment {
    if choice == ThemeChoice::Unnamed && env.settings.appearance.theme == Theme::System {
        env.settings.appearance.theme = Theme::Dark;
    }
    env
}

/// The look of a window of temor's, live: the person's look and the desktop's preferences from
/// `source`, a change re-theming the window, dark unless the person's file names a theme (the file
/// is read again when it changes).
pub fn use_window_look(source: &AppearanceSource, spawner: Arc<dyn Spawner>) -> Memo<Environment> {
    let stored = use_environment(source.store.clone(), source.system.clone(), spawner);
    let choice = use_memo({
        let store = source.store.clone();
        move || {
            let _ = stored();
            read_theme_choice(store.path::<AppearanceFile>().as_deref())
        }
    });
    use_memo(move || dark_by_default(stored(), choice()))
}

/// The palette the root with `appearance`, `look` and `system` resolves to.
pub fn resolved_palette(appearance: Appearance, look: &SpaceLook, system: SystemPrefs) -> Palette {
    let resolved = resolve(appearance, look.theme, system);
    palette_of(resolved.scheme, resolved.accent)
}

/// The grid's palette in `scheme` with `accent`, from quire's terminal tokens.
pub fn palette_of(scheme: Scheme, accent: Accent) -> Palette {
    // The ground is opaque (a solid token), so it is the back the translucent tokens lay over.
    let back = opaque(TerminalColour::Bg.value(scheme, accent), Hex([0, 0, 0]));
    let value = |token: TerminalColour| opaque(token.value(scheme, accent), back);
    let bg = value(TerminalColour::Bg);
    let mut ansi = [Rgb::default(); 16];
    for (slot, token) in ansi.iter_mut().zip(TerminalColour::ANSI) {
        *slot = to_rgb(value(token));
    }
    Palette {
        ansi,
        foreground: to_rgb(value(TerminalColour::Fg)),
        background: to_rgb(bg),
        cursor: to_rgb(value(TerminalColour::Cursor)),
        selection: to_rgb(value(TerminalColour::Selection)),
    }
}

/// A token as an opaque colour: a translucent one blended over `back`.
fn opaque(colour: Colour, back: Hex) -> Hex {
    match colour {
        Colour::Solid(hex) => hex,
        Colour::Alpha(hex, alpha) => hex.over(alpha, back),
    }
}

fn to_rgb(hex: Hex) -> Rgb {
    Rgb::new(hex.0[0], hex.0[1], hex.0[2])
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
