//! The crate's error: what a settings line or a key chord can get wrong.

use thiserror::Error;

/// Why outside settings text was refused. Each variant names the fix a person can make.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// The chord belongs to the desktop (Super, or Ctrl+Space); a keymap may not bind it.
    #[error("{0} belongs to the desktop")]
    ReservedChord(String),
    /// The chord already runs another action.
    #[error("{chord} is already used by {by}")]
    ChordInUse { chord: String, by: String },
    /// A settings line names no setting this version knows.
    #[error("no setting called {0:?}")]
    UnknownSetting(String),
    /// A settings line has a value its setting cannot take.
    #[error("{key} cannot be {value:?}")]
    BadSettingValue { key: String, value: String },
    /// A key chord in a settings file did not parse ("ctrl+shift+c" is the form).
    #[error("not a key combination: {0:?}")]
    BadChord(String),
}
