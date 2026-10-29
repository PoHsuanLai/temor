//! The window's panes and what happens to them (PLAN §7): one session per pane, the workspace
//! model that says which panes exist, and the edge that carries out what the model asks (start a
//! pane, take its scrollback, end it, save the session). Dioxus is not used here; the views read
//! the hub and send it the person's requests.

mod act;
pub mod agents;
mod bar;
mod bar_action;
mod bar_local;
mod bar_run;
mod blink_tick;
mod desk;
mod desktop;
mod desktop_io;
#[cfg(test)]
mod desktop_tests;
mod dispatch;
#[cfg(test)]
mod dispatch_tests;
#[cfg(test)]
mod drop_pane_tests;
mod events;
mod facts;
mod history;
mod hosts;
mod icons;
mod keeping;
mod keys;
mod local;
mod model;
mod narrow;
mod offer;
mod offer_reply;
#[cfg(test)]
mod open_tab_tests;
mod peek;
pub mod peek_step;
// Plugins in the hub: a child module so it reaches the hub's fields (the file lives with them).
#[path = "../plugins/glue.rs"]
mod plugin_glue;
mod quick;
mod remote;
mod remote_attach;
mod remote_consent;
mod remote_desk;
mod remote_edit;
mod remote_keep;
mod remote_line;
mod remote_loop;
mod remote_open;
mod remote_retry;
mod remote_step;
mod remote_view;
mod rows;
mod run;
mod say;
mod settings_live;
mod shelf;
mod space_edits;
mod space_launch;
mod space_move;
mod space_route;
mod space_rows;
mod spaces;
#[cfg(test)]
mod split_order_tests;
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
mod stage;
#[cfg(test)]
mod stage_tests;
mod start_folder;
mod tab_name;
#[cfg(test)]
mod tests;
mod transfers;
mod typing;
mod uploads;
mod viewer_changes;
mod viewer_diff;
mod viewer_host;
mod viewer_requests;
#[cfg(test)]
mod viewer_tests;
mod viewers;

pub use agents::{Agents, Alert, WaitingCount, spawn_agent_loops};
pub use bar::{BarData, WindowAction};
pub use bar_action::select_tab;
pub use blink_tick::spawn_blink_loop;
pub use desk::{Desk, HostOf, NEW_FOLDER};
pub use desktop::spawn_desktop_loop;
pub use dispatch::Origin;
pub use events::spawn_event_loop;
pub use facts::spawn_facts_loop;
pub use hosts::HostRows;
pub use icons::spawn_icons_loop;
pub use keeping::spawn_save_loop;
pub use keys::{spawn_keys_loop, workspace_input};
pub use model::{Hub, HubRef};
pub use offer::spawn_offer_loop;
pub use plugin_glue::spawn_plugins_loop;
pub use remote_loop::spawn_remote_loop;
pub use rows::{ConsoleRow, DragKey, FolderRows, RowIcon, SidebarData, TabRow, Tile, Unread};
pub use run::Run;
pub use say::refusal_text;
pub use settings_live::spawn_settings_loop;
pub use shelf::{Ask, AskKind, AskView, PaneTarget};
pub use space_edits::spawn_space_edits;
pub use space_rows::{KitSpaces, PendingSwitch};
pub use ssh_events::spawn_ssh_loop;
pub use stage::{Address, StageView, folder_url, page_path, temor_page, temor_page_name};
pub use temor_client::remote::session::{PortRow, RemoteStage, RemoteView};
pub use temor_client::ssh::view::{
    CheckRow, Direct, EditLine, EditStatus, ForwardLine, HostView, Stage,
};
pub use viewer_host::spawn_viewer_loop;
pub use viewers::ViewerShow;
