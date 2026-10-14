//! The one-time conversion of `terminal.conf` (PLAN §14) into `settings.toml` and `keys.conf`.
//! The old file is renamed `terminal.conf.old`, never deleted. A line that cannot be read is kept
//! as a comment beside where it would have gone, and reported.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use temor_pane::SettingsError;
use temor_pane::settings::parse_settings;

use super::convert::from_terminal;
use super::doc::TerminalDoc;
use super::keys_file::write_keys;
use crate::edge::Ambient;
use crate::error::UiError;

/// What `terminal.conf`'s text becomes.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    /// The settings lines, as the document `settings.toml` holds.
    pub doc: TerminalDoc,
    /// The text of `keys.conf`: the key lines in chordkit's form, then the refused key lines as
    /// comments.
    pub keys_text: String,
    /// Comment lines for the end of `settings.toml`: the refused settings lines.
    pub settings_notes: String,
    /// The refused lines with their 1-based numbers in `terminal.conf`.
    pub issues: Vec<(usize, SettingsError)>,
}

/// Converts the text of a `terminal.conf`. Pure.
pub fn convert_terminal_conf(text: &str) -> Converted {
    let parsed = parse_settings(text);
    let mut keys_text = String::from("# Your key changes, moved here from terminal.conf.\n");
    keys_text.push_str(&parsed.settings.keys.render());
    let mut settings_notes = String::new();
    for (line, issue) in &parsed.issues {
        let original = text
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("")
            .trim();
        let note = format!("# not read from terminal.conf ({issue}): {original}\n");
        match issue {
            SettingsError::Key(_) => keys_text.push_str(&note),
            _ => settings_notes.push_str(&note),
        }
    }
    Converted {
        doc: from_terminal(&parsed.settings),
        keys_text,
        settings_notes,
        issues: parsed.issues,
    }
}

/// Converts the old `terminal.conf` when there is one and no `settings.toml` yet. Returns the
/// lines worth telling the person; nothing to convert is nothing to tell.
///
/// The keys file goes first and the old file is renamed last, so a stop half way converts again
/// next time and loses nothing. A `keys.conf` that already exists is left as it is.
pub fn migrate_terminal_conf(ambient: &Ambient) -> Result<Vec<String>, UiError> {
    let store = ambient.settings_store();
    let old = ambient.old_settings_path();
    if store
        .path::<TerminalDoc>()
        .is_some_and(|path| path.exists())
    {
        return Ok(Vec::new());
    }
    let text = match std::fs::read_to_string(&old) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(UiError::SettingsRead(source)),
    };
    let converted = convert_terminal_conf(&text);
    let keys = ambient.keys_path();
    if !keys.exists() {
        write_keys(&keys, &converted.keys_text)?;
    }
    store.save(&converted.doc).map_err(UiError::SettingsWrite)?;
    if let Some(path) = store.path::<TerminalDoc>() {
        append(&path, &converted.settings_notes)?;
    }
    let kept = unused_old_name(&old);
    std::fs::rename(&old, &kept).map_err(UiError::SettingsRead)?;
    let mut said = vec![format!(
        "your settings were moved to settings.toml and keys.conf; the old file is kept as {}",
        kept.display()
    )];
    said.extend(
        converted
            .issues
            .iter()
            .map(|(line, issue)| format!("terminal.conf line {line}: {issue}")),
    );
    Ok(said)
}

/// `terminal.conf.old`, or the first `terminal.conf.old.N` that is free: nothing is overwritten.
fn unused_old_name(old: &Path) -> PathBuf {
    let named = |suffix: &str| {
        let mut name = old.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    };
    std::iter::once(named(".old"))
        .chain((1..).map(|n| named(&format!(".old.{n}"))))
        .find(|path| !path.exists())
        .unwrap_or_else(|| named(".old"))
}

/// Adds `text` to the end of the file at `path`; nothing for empty text.
fn append(path: &Path, text: &str) -> Result<(), UiError> {
    if text.is_empty() {
        return Ok(());
    }
    let io = |source: std::io::Error| {
        UiError::SettingsWrite(ds_settings::SettingsError::Io {
            path: path.to_path_buf(),
            source,
        })
    };
    let mut file = OpenOptions::new().append(true).open(path).map_err(io)?;
    file.write_all(text.as_bytes()).map_err(io)
}
