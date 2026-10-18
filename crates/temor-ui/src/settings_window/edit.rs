//! A change made in the Settings window to `settings.toml`: the file is read again, the edit is
//! applied to what it says now (so a change made meanwhile elsewhere is kept), and the result is
//! written back. The live follow (`hub::settings_live`) then puts it in force in the panes; the
//! window never applies a setting itself.

use std::path::PathBuf;

use ds_settings_page::SettingEdit;

use crate::edge::Ambient;
use crate::error::UiError;
use crate::settings::TerminalDoc;

/// `doc` with `edit` applied, or in words why a setting cannot hold the value.
pub fn applied(doc: &TerminalDoc, edit: &SettingEdit) -> Result<TerminalDoc, UiError> {
    let values =
        toml::Value::try_from(doc).map_err(|why| UiError::SettingsRefused(why.to_string()))?;
    edit.applied_to(values)
        .try_into::<TerminalDoc>()
        .map_err(|why| UiError::SettingsRefused(refusal(edit, why.message())))
}

/// What the person reads when `edit` is refused: the setting, then what it takes.
fn refusal(edit: &SettingEdit, why: &str) -> String {
    let key = &edit.path.0;
    format!("{key} was not changed: {why}")
}

/// What a saved edit left: the document the file holds, and where an unreadable file was set
/// aside to make room for it, if one was.
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    pub doc: TerminalDoc,
    pub kept: Option<PathBuf>,
}

/// Reads `settings.toml` as it is now, applies `edit` and saves it. A file that cannot be written
/// is an error and nothing changes.
pub fn save_edit(ambient: &Ambient, edit: &SettingEdit) -> Result<Edited, UiError> {
    let store = ambient.settings_store();
    let current = store.load::<TerminalDoc>().value;
    let next = applied(&current, edit)?;
    let kept = if next == current {
        None
    } else {
        store.save(&next).map_err(UiError::SettingsWrite)?.kept_at
    };
    Ok(Edited { doc: next, kept })
}

/// The document `settings.toml` holds now, as the window shows it.
pub fn current(ambient: &Ambient) -> TerminalDoc {
    ambient.settings_store().load::<TerminalDoc>().value
}

/// The document as TOML, for `SettingsPage`'s `values`.
pub fn values_of(doc: &TerminalDoc) -> toml::Value {
    toml::Value::try_from(doc).unwrap_or_else(|_| toml::Value::Table(toml::Table::new()))
}
