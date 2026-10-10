//! The window's Dioxus views: the root component, the sidebar of tabs, the tree of panes, one
//! pane's view with its find bar and paste confirmation, and the input method glue. Views read the
//! session's state and send it the person's events; they decide nothing themselves.

pub mod bar;
pub mod blocks;
pub mod confirm;
pub mod convert;
pub mod dnd;
pub mod find_bar;
pub mod handlers;
pub mod ime;
pub mod launch;
pub mod miniature;
pub mod palette;
pub mod pane_view;
pub mod pointer;
pub mod probe;
pub mod quick;
pub mod slot;
pub mod status;
pub mod tab_row;
pub mod tab_sidebar;
pub mod theme;
pub mod title;
pub mod tree_view;
pub mod window;
pub mod window_help;
pub mod window_style;

pub use launch::{Launch, SESSION_LEADER_ARG};
pub use window::App;
