//! Loading at launch: convert `terminal.conf` once, read `settings.toml` through the store and
//! `keys.conf` as a file, and hand the pane its typed settings.

use ds_settings::SettingsDoc;
use temor_pane::settings::TerminalSettings;

use super::doc::TerminalDoc;
use super::keys_file::read_overrides;
use super::migrate::migrate_terminal_conf;
use super::told::Told;
use crate::edge::Ambient;
use crate::error::UiError;

/// The settings in force at start, and what was worth telling the person about the files.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub settings: TerminalSettings,
    /// One line each, for the log: a setting the file holds that cannot be used, a key line that
    /// was refused, the conversion of the old file.
    pub notes: Vec<String>,
    /// The same, in plain words for the window.
    pub told: Told,
}

/// The settings under `ambient`'s config folder over the defaults. A missing file is the
/// defaults; a bad value costs only itself. Only a key file that cannot be read at all, or a
/// conversion that cannot be written, is an error.
pub fn load_settings(ambient: &Ambient) -> Result<Loaded, UiError> {
    let mut told = Told::default();
    let mut notes = Vec::new();
    if let Some(migrated) = migrate_terminal_conf(ambient)? {
        notes.extend(migrated.notes());
        told.migrated(migrated);
    }
    let loaded = ambient.settings_store().load::<TerminalDoc>();
    notes.extend(loaded.diagnostics(TerminalDoc::FILE));
    for key in &loaded.invalid {
        match key.path.as_str() {
            "" => told.unreadable(),
            path => told.refused(path),
        }
    }
    let (keys, key_issues) = read_overrides(&ambient.keys_path())?;
    told.key_issues(&key_issues);
    let (settings, problems) = loaded.value.terminal(keys);
    for problem in &problems {
        told.refused_value(problem);
    }
    notes.extend(
        problems
            .into_iter()
            .map(|issue| format!("settings.toml: {issue}")),
    );
    notes.extend(
        key_issues
            .into_iter()
            .map(|(line, issue)| format!("keys.conf line {line}: {issue}")),
    );
    Ok(Loaded {
        settings,
        notes,
        told,
    })
}
