//! The window's Dioxus views: the root component, the find bar, the paste confirmation, and the
//! input method glue. Views read the session's state and send it the person's events; they
//! decide nothing themselves.

pub mod confirm;
pub mod convert;
pub mod find_bar;
pub mod ime;
pub mod launch;
pub mod pointer;
pub mod probe;
pub mod slot;
pub mod window;

pub use launch::{Launch, SESSION_LEADER_ARG};
pub use window::App;
