//! temor's terminal pane, headless: the pure parts a terminal needs (PLAN §6, §11 P1). Key
//! chords and the keymap, settings text, cell rows, selection and search, paste rules, and the
//! link and working-directory checks, and the engine that runs a program on a pty and feeds its
//! terminal. The engine never spawns a thread: the caller runs [`PaneRunner::run`].
//!
//! # Example
//!
//! Settings text is read over the defaults; a bad line is reported and the rest still applies:
//!
//! ```
//! use temor_pane::keys::{Action, Chord};
//! use temor_pane::settings::parse_settings;
//!
//! let parsed = parse_settings("keys.find_next = ctrl+alt+g\nfont.size = huge\n");
//! assert_eq!(parsed.issues.len(), 1);
//! assert_eq!(
//!     parsed.settings.keymap.chord_for(Action::FindNext),
//!     Some(&Chord::parse("ctrl+alt+g").expect("a chord")),
//! );
//! ```

pub mod keys;
pub mod paste;
#[cfg(unix)]
pub mod report;
pub mod settings;
pub mod text;

#[cfg(feature = "ghostty")]
mod engine;
mod error;
#[cfg(unix)]
mod platform;
#[cfg(feature = "ghostty")]
mod shell;

#[cfg(feature = "ghostty")]
pub use engine::{
    EventSink, Hold, Output, PaneEvent, PaneHandle, PaneRunner, PaneSpec, Term, start,
    start_restored,
};
pub use error::{PaneError, SettingsError};
#[cfg(feature = "ghostty")]
pub use shell::{ShellPaneInput, shell_pane};
