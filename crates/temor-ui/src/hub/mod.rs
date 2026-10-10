//! The window's panes and what happens to them (PLAN §7): one session per pane, the workspace
//! model that says which panes exist, and the edge that carries out what the model asks (start a
//! pane, take its scrollback, end it, save the session). Dioxus is not used here; the views read
//! the hub and send it the person's requests.

mod act;
pub mod agents;
mod desk;
mod events;
mod keys;
mod model;
mod persist;
mod restore;
mod rows;
mod say;
#[cfg(test)]
mod tests;

pub use agents::{
    AgentMenu, AgentPick, Agents, Alert, LeftoverRow, SheetView, WaitingCount, Where,
    spawn_agent_loops,
};
pub use desk::Desk;
pub use events::spawn_event_loop;
pub use keys::workspace_input;
pub use model::{Hub, HubRef};
pub use persist::spawn_save_loop;
pub use rows::{DragKey, SidebarData, TabRow, Tile, Unread};
pub use say::refusal_text;
