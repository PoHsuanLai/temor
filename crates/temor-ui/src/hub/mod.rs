//! The window's panes and what happens to them (PLAN §7): one session per pane, the workspace
//! model that says which panes exist, and the edge that carries out what the model asks (start a
//! pane, take its scrollback, end it, save the session). Dioxus is not used here; the views read
//! the hub and send it the person's requests.

mod act;
pub mod agents;
mod bar;
mod bar_action;
mod bar_run;
#[cfg(test)]
mod bar_tests;
mod blink_tick;
mod desk;
mod desktop;
mod desktop_io;
#[cfg(test)]
mod desktop_tests;
mod events;
mod facts;
mod hosts;
mod keys;
mod model;
mod narrow;
mod offer;
mod offer_reply;
mod peek;
pub mod peek_step;
mod persist;
mod quick;
mod remote;
mod remote_attach;
mod remote_consent;
mod remote_desk;
mod remote_edit;
#[cfg(test)]
mod remote_edit_tests;
mod remote_keep;
mod remote_line;
mod remote_loop;
mod remote_open;
mod remote_retry;
mod remote_source;
mod remote_step;
mod remote_view;
mod restore;
mod rows;
mod running_tick;
mod say;
mod shelf;
mod space_launch;
mod space_move;
mod space_route;
mod space_rows;
mod spaces;
mod ssh;
mod ssh_check;
mod ssh_connect;
mod ssh_desk;
mod ssh_edit;
mod ssh_events;
mod ssh_forward;
mod ssh_master;
mod ssh_permits;
#[cfg(test)]
mod ssh_tests;
mod ssh_view;
mod ssh_wait;
mod tab_name;
#[cfg(test)]
mod tests;
mod transfers;
mod uploads;
mod viewer_changes;
mod viewer_diff;
mod viewer_host;
mod viewer_requests;
#[cfg(test)]
mod viewer_tests;
mod viewers;

pub use agents::{
    AgentMenu, AgentPick, Agents, Alert, LeftoverRow, PointLine, RestoreMenu, SheetView,
    WAIT_FOR_AGENT, WaitingCount, Where, spawn_agent_loops,
};
pub use bar::{BarData, WindowAction};
pub use blink_tick::spawn_blink_loop;
pub use desk::Desk;
pub use desktop::spawn_desktop_loop;
pub use events::spawn_event_loop;
pub use facts::spawn_facts_loop;
pub use hosts::HostRows;
pub use keys::{spawn_keys_loop, workspace_input};
pub use model::{Hub, HubRef};
pub use offer::spawn_offer_loop;
pub use persist::spawn_save_loop;
pub use remote_loop::spawn_remote_loop;
pub use remote_view::{PortRow, RemoteStage, RemoteView};
pub use rows::{DragKey, SidebarData, TabRow, Tile, Unread};
pub use running_tick::spawn_running_loop;
pub use say::refusal_text;
pub use shelf::{Ask, AskKind, AskView};
pub use space_rows::{KitSpaces, PendingSwitch};
pub use ssh_events::spawn_ssh_loop;
pub use ssh_view::{CheckRow, Direct, EditLine, EditStatus, ForwardLine, HostView, Stage};
pub use viewer_host::spawn_viewer_loop;
pub use viewers::ViewerShow;
