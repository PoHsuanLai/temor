//! Which keys are on which page of the Settings window, and which of them the window's own rows
//! draw because quire's `SettingsPage` shows them read-only.
//!
//! Pure over the schema.

use ds_settings::schema::{KeyKind, KeySpec, Schema};
use temor_pane::keys::SettingsPane;

use crate::settings::schema;

/// The page a section of the settings file is on.
///
/// TEMPORARY, and the only place that knows: `#[settings(page = ...)]` names the Settings app's
/// page, not a page of temor's own window, so General and Appearance are told apart by section
/// title. When quire adds a page id to the attribute, this is the one function to swap (every
/// other part of the window asks it, never a title). A section it does not know is General.
pub fn page_of(section: &str) -> SettingsPane {
    match section {
        "Text" | "Cursor" | "History" => SettingsPane::Appearance,
        _ => SettingsPane::General,
    }
}

/// How a key is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawnBy {
    /// Quire's `SettingsPage`: a switch, segments or a pop-up button.
    Page,
    /// The window's own row: text, a number or a list, which the page only shows.
    Window,
}

/// Who draws a key of `kind`.
pub fn drawn_by(kind: &KeyKind) -> DrawnBy {
    match kind {
        KeyKind::Toggle { .. }
        | KeyKind::Segmented { .. }
        | KeyKind::Menu { .. }
        | KeyKind::Fixed { .. } => DrawnBy::Page,
        KeyKind::Bounded { .. }
        | KeyKind::Text
        | KeyKind::List(_)
        | KeyKind::Colour
        | KeyKind::Shortcut
        | KeyKind::Rows { .. }
        | KeyKind::Live { .. } => DrawnBy::Window,
    }
}

/// The keys of `pane`, in the schema's order, drawn by `who`.
pub fn keys_of(pane: SettingsPane, who: DrawnBy) -> Vec<KeySpec> {
    schema()
        .key
        .into_iter()
        .filter(|key| page_of(&key.section.0) == pane && drawn_by(&key.kind) == who)
        .collect()
}

/// The schema `SettingsPage` draws for `pane`: only the keys it can draw as controls.
pub fn page_schema(pane: SettingsPane) -> Schema {
    Schema {
        key: keys_of(pane, DrawnBy::Page),
        ..schema()
    }
}

/// The section titles of `pane`'s keys the window draws itself, in the schema's order, each with
/// its keys.
pub fn own_sections(pane: SettingsPane) -> Vec<(String, Vec<KeySpec>)> {
    let mut out: Vec<(String, Vec<KeySpec>)> = Vec::new();
    for key in keys_of(pane, DrawnBy::Window) {
        match out.iter_mut().find(|(title, _)| *title == key.section.0) {
            Some((_, keys)) => keys.push(key),
            None => out.push((key.section.0.clone(), vec![key])),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use temor_pane::keys::SettingsPane;

    use super::*;

    /// Every key of the schema is on exactly one of the two pages, and each is drawn by someone:
    /// a new section that `page_of` does not know would land on General, never vanish.
    #[test]
    fn every_key_is_on_one_page_and_drawn_once() {
        let total = schema().key.len();
        let mut seen = 0;
        for pane in [SettingsPane::General, SettingsPane::Appearance] {
            let page = keys_of(pane, DrawnBy::Page).len();
            let own: usize = own_sections(pane).iter().map(|(_, keys)| keys.len()).sum();
            seen += page + own;
        }
        assert_eq!(seen, total);
        for pane in [SettingsPane::Keyboard, SettingsPane::Spaces] {
            assert!(keys_of(pane, DrawnBy::Page).is_empty(), "{pane:?}");
            assert!(own_sections(pane).is_empty(), "{pane:?}");
        }
    }

    #[test]
    fn the_cursor_is_a_control_and_the_font_size_is_the_windows_row() {
        let on = |pane, who: DrawnBy| -> Vec<String> {
            keys_of(pane, who).into_iter().map(|k| k.path.0).collect()
        };
        assert_eq!(
            on(SettingsPane::Appearance, DrawnBy::Page),
            ["cursor.style", "cursor.blink"]
        );
        let own = on(SettingsPane::Appearance, DrawnBy::Window);
        assert!(own.contains(&"font.size".to_owned()), "{own:?}");
        let general = on(SettingsPane::General, DrawnBy::Window);
        assert!(general.contains(&"offer.add".to_owned()), "{general:?}");
    }
}
