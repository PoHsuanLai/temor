//! temor's window (PLAN §2, lane app), as the binary launches it: the window's run and the shells
//! it starts. The window is four crates, each naming only those below it: `temor-ui-base` (what
//! it is launched with, the settings file, the edges, a pane's slot), `temor-hub` (its state),
//! `temor-views` (its components) and this one, which joins them and opens the window.

pub mod app;

pub use app::run;
pub use temor_ui_base::edge::{Ambient, load_keymap};
pub use temor_ui_base::launch::{FirstWords, Launch, LauncherChoice, SESSION_LEADER_ARG, TakeOnce};
pub use temor_ui_base::settings::{load_settings, write_schema};
pub use temor_ui_base::theme::AppearanceSource;
