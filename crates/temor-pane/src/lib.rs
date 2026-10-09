//! temor's terminal pane, headless: the pure parts a terminal needs (PLAN §6, §11 P1). Key
//! chords and the keymap, settings text, cell rows, selection and search, paste rules, and the
//! link and working-directory checks, and the engine that runs a program on a pty and feeds its
//! terminal. The engine never spawns a thread: the caller runs [`PaneRunner::run`].

pub mod keys;
pub mod paste;
pub mod settings;
pub mod text;

mod engine;
mod error;
mod shell;

pub use engine::{
    EventSink, Hold, Output, PaneEvent, PaneHandle, PaneRunner, PaneSpec, Term, start,
};
pub use error::{PaneError, SettingsError};
pub use shell::{ShellPaneInput, shell_pane};
