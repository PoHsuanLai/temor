//! temor's terminal pane, headless: the pure parts a terminal needs (PLAN §6, §11 P1). Key
//! actions and their keymap defaults, settings text, cell rows, selection and search, paste rules, and the
//! link and working-directory checks, and the engine that runs a program on a pty and feeds its
//! terminal. The engine never spawns a thread: the caller runs [`PaneRunner::run`].
//!
//! # Example
//!
//! Settings text is read over the defaults; a bad line is reported and the rest still applies.
//! Key lines are chordkit's form and are kept as the person's changes to the keymap:
//!
//! ```
//! use temor_pane::settings::parse_settings;
//!
//! let parsed = parse_settings("temor.split-right = Primary+Alt+N\nfont.size = huge\n");
//! assert_eq!(parsed.issues.len(), 1);
//! assert_eq!(parsed.settings.keys.entries().len(), 1);
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
#[cfg(all(unix, feature = "engine"))]
mod platform;
#[cfg(feature = "ghostty")]
mod shell;

#[cfg(feature = "ghostty")]
pub use engine::{
    ByteTap, ClipboardMirror, EventSink, FedSpec, Feeder, OfferId, OfferKind, Output, PaintSource,
    PaneEvent, PaneHandle, PaneLink, PaneRunner, PaneSpec, ReadPolicy, RenderHold, SeenForeground,
    Term, TransferEnd, TransferEvent, TransferSetup, start, start_fed, start_restored,
};
pub use error::{PaneError, SettingsError};
#[cfg(feature = "ghostty")]
pub use shell::{ShellPaneInput, cursor_shape, shell_pane};
// The command line of a foreground process, as the engine's events carry it.
#[cfg(feature = "engine")]
pub use temor_pty::CommandLine;
