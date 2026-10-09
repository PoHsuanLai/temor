//! temor's terminal pane, headless: the pure parts a terminal needs (PLAN §6, §11 P1). Key
//! chords and the keymap, settings text, cell rows, selection and search, paste rules, and the
//! link and working-directory checks. No I/O, no threads, no UI and no foreign code; the
//! engine that runs a program on a pty builds on these.

pub mod keys;
pub mod paste;
pub mod settings;
pub mod text;

mod error;

pub use error::SettingsError;
