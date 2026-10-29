//! The notice vocabulary: what the person can be told, and the setting that decides when a
//! finished command is worth telling.

use crate::block::Outcome;

/// What the person can be told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// A command ran long enough and finished while the pane was unfocused.
    CommandFinished {
        /// How it ended.
        outcome: Outcome,
        /// How long it ran, in milliseconds.
        took_ms: u64,
        /// The command line, as typed.
        command: String,
    },
    /// The program asked for a notification (OSC 9, 99 or 777). `title` is empty when the
    /// protocol had none.
    Program { title: String, body: String },
    /// The bell rang while the pane was unfocused.
    Bell,
}
