//! What is kept of a pane when its window goes away: the state a new shell needs to look like the
//! old one (folder, variables, aliases; never a secret's value) and the commands it ran. Written as
//! JSON, versioned, read leniently: a newer file's extra fields are ignored and a damaged field is
//! left out rather than refusing the whole file. The screen is kept apart (a terminal snapshot has
//! no compatibility promise across terminal versions).

use serde::{Deserialize, Deserializer, Serialize};

use super::shell_state::{Replay, holds_secret, is_secret, looks_secret, never_carried};
use crate::block::Block;

/// The version this build writes.
pub const VERSION: u32 = 1;

/// The most commands a checkpoint keeps (the newest).
pub const MOST_HISTORY: usize = 500;

/// One command that ran, without where it sat on the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRecord {
    pub command: String,
    #[serde(default)]
    pub exit: Option<i32>,
    #[serde(default)]
    pub took_ms: Option<u64>,
}

/// A pane's kept state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    #[serde(default)]
    pub version: u32,
    #[serde(default, deserialize_with = "lenient")]
    pub cwd: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub env: Vec<(String, String)>,
    #[serde(default, deserialize_with = "lenient")]
    pub aliases: Vec<(String, String)>,
    /// Secrets the old shell had, by name only.
    #[serde(default, deserialize_with = "lenient")]
    pub withheld: Vec<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub history: Vec<CommandRecord>,
    /// The session the pane had in the local daemon when temor quit; the next launch asks the
    /// daemon for it.
    #[serde(default, deserialize_with = "lenient")]
    pub daemon_session: Option<u64>,
    /// The report key of that session's shell, so the next launch knows its reports. Only with a
    /// daemon session; the file is the person's alone (0600 in a private folder).
    #[serde(default, deserialize_with = "lenient")]
    pub report_key: Option<String>,
}

/// A field that cannot be read is its default.
fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// Whether a command goes into a checkpoint's history: not one typed with a space first (the
/// shells' way to keep a command out of history), and not one that sets a secret or holds
/// something that looks like one.
pub fn kept_in_history(command: &str) -> bool {
    !command.starts_with(' ') && !holds_secret(command)
}

/// Why a checkpoint was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckpointFault {
    /// Not JSON, or not an object.
    Damaged,
    /// Written by a newer build with a layout this one cannot know.
    Newer(u32),
}

impl Checkpoint {
    /// A checkpoint of `replay` and the commands in `blocks` (oldest first; the newest
    /// [`MOST_HISTORY`] are kept; a command still running is left out, and so is one that is
    /// kept out of history: see [`kept_in_history`]).
    pub fn of<'a>(replay: Replay, blocks: impl Iterator<Item = &'a Block>) -> Self {
        let mut history: Vec<CommandRecord> = blocks
            .filter(|block| block.ended.is_some() && kept_in_history(&block.command.0))
            .map(|block| CommandRecord {
                command: block.command.0.clone(),
                exit: block.exit.map(|code| code.0),
                took_ms: block.elapsed_ms(),
            })
            .collect();
        let extra = history.len().saturating_sub(MOST_HISTORY);
        history.drain(..extra);
        Self {
            version: VERSION,
            cwd: replay.cwd,
            env: replay.env,
            aliases: replay.aliases,
            withheld: replay.withheld,
            history,
            daemon_session: None,
            report_key: None,
        }
    }

    /// This checkpoint's commands after `earlier` ones (those of a shell this one continues),
    /// the newest [`MOST_HISTORY`] kept.
    pub fn after(mut self, earlier: &[CommandRecord]) -> Self {
        let mut all = earlier.to_vec();
        all.append(&mut self.history);
        let extra = all.len().saturating_sub(MOST_HISTORY);
        all.drain(..extra);
        self.history = all;
        self
    }

    /// What a new shell is told. A file written by an earlier build may hold what this one
    /// would not keep; that is left out here.
    pub fn replay(&self) -> Replay {
        Replay {
            cwd: self.cwd.clone(),
            env: self
                .env
                .iter()
                .filter(|(name, value)| {
                    !never_carried(name) && !is_secret(name) && !looks_secret(value)
                })
                .cloned()
                .collect(),
            aliases: self
                .aliases
                .iter()
                .filter(|(_, value)| !holds_secret(value))
                .cloned()
                .collect(),
            withheld: self.withheld.clone(),
        }
    }

    /// The file's text.
    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// The checkpoint in a file's text.
    pub fn decode(text: &str) -> Result<Self, CheckpointFault> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|_| CheckpointFault::Damaged)?;
        if !value.is_object() {
            return Err(CheckpointFault::Damaged);
        }
        let found: Self = serde_json::from_value(value).map_err(|_| CheckpointFault::Damaged)?;
        match found.version {
            0 => Err(CheckpointFault::Damaged),
            version if version > VERSION => Err(CheckpointFault::Newer(version)),
            _ => Ok(found),
        }
    }
}

#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;
