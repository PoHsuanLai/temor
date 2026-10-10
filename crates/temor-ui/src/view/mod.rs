//! The window's Dioxus views: the root component, the sidebar of tabs, the tree of panes, one
//! pane's view with its find bar and paste confirmation, and the input method glue. Views read the
//! session's state and send it the person's events; they decide nothing themselves.

pub mod bar;
pub mod block_header;
pub mod block_tools;
pub mod blocks;
pub mod card_colours;
pub mod confirm;
pub mod convert;
pub mod dnd;
pub mod fact_chips;
pub mod find_bar;
pub mod handlers;
pub mod host_banners;
pub mod host_list;
pub mod host_words;
pub mod ime;
pub mod launch;
pub mod miniature;
pub mod offer;
pub mod palette;
pub mod pane_focus;
pub mod pane_footer;
pub mod pane_keys;
pub mod pane_pointer;
pub mod pane_view;
pub mod peek;
pub mod peek_card;
pub mod peek_keys;
pub mod pointer;
pub mod probe;
pub mod quick;
pub mod radar;
pub mod remote;
pub mod remote_launch;
pub mod remote_words;
pub mod shelf;
pub mod slot;
pub mod space_handle;
pub mod space_ui;
pub mod status;
pub mod tab_row;
pub mod tab_sidebar;
pub mod theme;
pub mod title;
pub mod transfer_ask;
pub mod tree_view;
pub mod viewer_open;
pub mod viewer_pane;
pub mod window;
pub mod window_help;
pub mod window_style;

pub use launch::{Launch, SESSION_LEADER_ARG};
pub use window::App;
