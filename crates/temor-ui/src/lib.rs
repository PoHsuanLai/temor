//! temor's window (PLAN §2, lane app): the terminal pane, its input and painting, and the app's
//! edges: the pane's PTY I/O thread, the settings file, and the opener. Pure parts live in
//! `temor-core`; this crate holds what touches the OS, the emulator and Dioxus.

pub mod app;
pub mod bar;
pub mod edge;
pub mod error;
pub mod hub;
pub mod plugins;
pub mod settings;
pub mod settings_window;
pub mod view;

// How a pane is shown is its own crate; the window names its parts as its own.
pub use temor_pane_view::{input, paint, pane, platform, session, words};

pub use app::run;
pub use view::launch::LauncherChoice;
pub use view::theme::AppearanceSource;
pub use view::{FirstWords, Launch, SESSION_LEADER_ARG, TakeOnce};
