//! Reads the theme choice from quire's `appearance.toml`. (temor's own settings load in
//! `crate::settings`.)

use std::path::Path;

/// Whether the person has chosen a theme in `appearance.toml`. Quire's typed file cannot say: a
/// missing `theme` and `theme = "system"` both read as "follow the desktop".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    /// `[appearance] theme` is `system`, `light` or `dark`.
    Named,
    /// No file, no key, or a word that is none of the three: temor's own default applies (dark).
    Unnamed,
}

/// Whether `text` (the contents of `appearance.toml`, or `None` without a file) names a theme.
pub fn theme_choice(text: Option<&str>) -> ThemeChoice {
    let word = text
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|table| {
            table
                .get("appearance")?
                .as_table()?
                .get("theme")?
                .as_str()
                .map(str::to_ascii_lowercase)
        });
    match word.as_deref() {
        Some("system" | "light" | "dark") => ThemeChoice::Named,
        _ => ThemeChoice::Unnamed,
    }
}

/// [`theme_choice`] of the file at `path`; an unreadable file names nothing.
pub fn read_theme_choice(path: Option<&Path>) -> ThemeChoice {
    let text = path.and_then(|path| std::fs::read_to_string(path).ok());
    theme_choice(text.as_deref())
}
