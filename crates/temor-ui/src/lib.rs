//! temor's window (PLAN §2, lane app): the terminal pane, its input and painting, and the app's
//! edges: the pane's PTY I/O thread, the settings file, and the opener. Pure parts live in
//! `temor-core`; this crate holds what touches the OS, the emulator and Dioxus.

pub mod app;
pub mod edge;
pub mod error;
pub mod input;
pub mod paint;
pub mod pane;
pub mod platform;
pub mod session;
pub mod view;

pub use app::run;
pub use view::launch::LauncherChoice;
pub use view::theme::AppearanceSource;
pub use view::{Launch, SESSION_LEADER_ARG};
