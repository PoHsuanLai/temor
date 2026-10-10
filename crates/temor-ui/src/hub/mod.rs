//! The window's panes and what happens to them (PLAN §7): one session per pane, the workspace
//! model that says which panes exist, and the edge that carries out what the model asks (start a
//! pane, take its scrollback, end it, save the session). Dioxus is not used here; the views read
//! the hub and send it the person's requests.

mod act;
mod desk;
mod events;
mod hosts;
mod keys;
mod model;
mod persist;
mod restore;
mod rows;
mod say;
mod ssh;
mod ssh_check;
mod ssh_connect;
mod ssh_desk;
mod ssh_edit;
mod ssh_events;
mod ssh_forward;
mod ssh_master;
#[cfg(test)]
mod ssh_tests;
mod ssh_view;
#[cfg(test)]
mod tests;

pub use desk::Desk;
pub use events::spawn_event_loop;
pub use hosts::{HostRows, NoTailnet, TailnetFeed, TailnetHandle};
pub use keys::workspace_input;
pub use model::{Hub, HubRef};
pub use persist::spawn_save_loop;
pub use rows::{DragKey, SidebarData, TabRow, Tile, Unread};
pub use say::refusal_text;
pub use ssh_events::spawn_ssh_loop;
pub use ssh_view::{CheckRow, Direct, EditLine, EditStatus, ForwardLine, HostView, Stage};
