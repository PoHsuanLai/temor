//! What the person is told about their settings when temor starts, in plain words. The log keeps
//! the exact lines (`Loaded::notes`); this names a setting as the Settings window labels it and
//! says what temor did about it, never the file's keys or a reader's error.

use std::collections::BTreeSet;
use std::path::Path;

use temor_panes::pane::SettingsError;
use temor_panes::pane::keys::KeyIssue;

use super::kept::kept_aside;
use super::migrate::Migrated;
use super::schema::schema;

/// What was worth telling the person about their settings files at start.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Told {
    /// `settings.toml` is there and nothing in it could be read.
    unreadable: bool,
    /// The settings whose value could not be used, by their labels, in the order found.
    refused: Vec<String>,
    /// The `keys.conf` lines that could not be used.
    key_lines: BTreeSet<usize>,
    migrated: Option<Migrated>,
}

impl Told {
    /// `settings.toml` could not be read at all.
    pub(super) fn unreadable(&mut self) {
        self.unreadable = true;
    }

    /// The setting at `path` (`font.size`) could not be used.
    pub(super) fn refused(&mut self, path: &str) {
        let label = label_of(path);
        if !self.refused.contains(&label) {
            self.refused.push(label);
        }
    }

    /// A problem the settings reader found in a value; only a refused value is the person's.
    pub(super) fn refused_value(&mut self, problem: &SettingsError) {
        if let SettingsError::BadSettingValue { key, .. } = problem {
            self.refused(key);
        }
    }

    pub(super) fn migrated(&mut self, migrated: Migrated) {
        self.migrated = Some(migrated);
    }

    /// The `keys.conf` lines among `issues` (line 0 is the desktop's own keys, not the person's).
    pub fn key_issues(&mut self, issues: &[KeyIssue]) {
        self.key_lines.extend(
            issues
                .iter()
                .map(|(line, _)| *line)
                .filter(|line| *line > 0),
        );
    }

    /// One sentence for each thing worth telling, empty when every setting is in force.
    pub fn sentences(&self) -> Vec<String> {
        let mut said = Vec::new();
        if let Some(migrated) = &self.migrated {
            said.push(format!(
                "temor moved your settings to a new file and kept the old one as {}.",
                file_name(&migrated.kept)
            ));
            said.extend(migrated.set_aside.as_deref().map(kept_aside));
            said.extend(match migrated.issues.len() {
                0 => None,
                1 => Some("One old setting couldn't be carried over.".to_owned()),
                n => Some(format!("{n} old settings couldn't be carried over.")),
            });
        }
        if self.unreadable {
            said.push(
                "Your settings couldn't be read, so temor started with its default settings. \
                 Changing a setting starts a fresh file and keeps the old one."
                    .to_owned(),
            );
        }
        said.extend(match self.refused.as_slice() {
            [] => None,
            [one] => Some(format!(
                "“{one}” couldn't be used as you set it, so it's back to its default."
            )),
            many => Some(format!(
                "Some settings couldn't be used as you set them, so they're back to their \
                 defaults: {}.",
                many.join(", ")
            )),
        });
        said.extend(match self.key_lines.len() {
            0 => None,
            1 => Some(
                "One of your keyboard shortcuts couldn't be used, so that action keeps its usual \
                 keys."
                    .to_owned(),
            ),
            n => Some(format!(
                "{n} of your keyboard shortcuts couldn't be used, so those actions keep their \
                 usual keys."
            )),
        });
        said
    }
}

/// What the Settings window calls the setting at `path`; a key without a label (none today)
/// is named by its last part.
fn label_of(path: &str) -> String {
    schema()
        .key
        .into_iter()
        .find(|key| key.path.0 == path)
        .map(|key| key.label.0)
        .unwrap_or_else(|| path.rsplit('.').next().unwrap_or(path).replace('_', " "))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
