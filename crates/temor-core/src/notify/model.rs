//! The notice vocabulary: what the person can be told, and the setting that decides when a
//! finished command is worth telling.

use crate::block::Outcome;
use ds_core::word::Word;

/// Whether the pane has the keyboard. A notice only goes out while it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum Focus {
    Focused,
    Unfocused,
}

/// How long a command must run before its finish is worth a notice. `Off` never notifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotifyAfter {
    Off,
    /// A command running at least this many seconds.
    Seconds(u32),
}

impl NotifyAfter {
    /// The threshold when the settings say nothing: ten seconds (PLAN §6, the notify setting).
    pub const DEFAULT: Self = Self::Seconds(10);

    /// The threshold in milliseconds; `None` when off.
    pub fn threshold_ms(self) -> Option<u64> {
        match self {
            Self::Off => None,
            Self::Seconds(s) => Some(u64::from(s) * 1000),
        }
    }
}

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
