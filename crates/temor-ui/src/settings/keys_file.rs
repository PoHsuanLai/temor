//! `temor/keys.conf`: the person's key changes in chordkit's line form
//! (`temor.split-right = Primary+Alt+N`, `copy = none`), in a file of their own. Reading is pure
//! over the text; the file's edges are here too.

use std::path::Path;

use chordkit::Overrides;
use temor_pane::SettingsError;
use temor_pane::keys::{KeyIssue, is_key_action};

use crate::error::UiError;

/// The changes `text` holds, with the lines that were refused (1-based; each key line that names
/// no action of temor's, a chord that cannot be read, a chord the system keeps).
pub fn overrides_of(text: &str) -> (Overrides, Vec<KeyIssue>) {
    let mut issues: Vec<KeyIssue> = Vec::new();
    let mut kept = String::new();
    for (index, raw) in text.lines().enumerate() {
        match foreign_key(raw) {
            Some(key) => issues.push((
                index + 1,
                SettingsError::Key(format!("no action called {key}")),
            )),
            None => kept.push_str(raw),
        }
        // Blank in place of a line left out, so the reader's line numbers are the file's.
        kept.push('\n');
    }
    let (overrides, problems) = Overrides::parse(&kept);
    issues.extend(
        problems
            .into_iter()
            .map(|problem| (problem.line, SettingsError::Key(problem.what))),
    );
    issues.sort_by_key(|(line, _)| *line);
    (overrides, issues)
}

/// The action a key line names when it is not one temor has (another app's, or a typo).
fn foreign_key(raw: &str) -> Option<&str> {
    let (key, _) = raw.split_once('=')?;
    let key = key.trim();
    (!key.starts_with('#') && !is_key_action(key)).then_some(key)
}

/// The changes in the file at `path`; none when there is no file.
pub fn read_overrides(path: &Path) -> Result<(Overrides, Vec<KeyIssue>), UiError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(overrides_of(&text)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(overrides_of("")),
        Err(source) => Err(UiError::SettingsRead(source)),
    }
}

/// The file's text at `path`; empty when there is no file.
pub fn read_keys_text(path: &Path) -> Result<String, UiError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(source) => Err(UiError::SettingsRead(source)),
    }
}

/// Writes `text` as the key file at `path`, making its folder when it is missing. The text lands
/// in a temporary file beside it and is renamed into place, so a watcher never reads a part.
pub fn write_keys(path: &Path, text: &str) -> Result<(), UiError> {
    let io = |at: &Path| {
        let path = at.to_path_buf();
        move |source| UiError::SettingsWrite(ds_settings::SettingsError::Io { path, source })
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io(dir))?;
    }
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, text).map_err(io(&tmp))?;
    std::fs::rename(&tmp, path).map_err(io(path))
}
