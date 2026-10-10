//! The workspace's pure model (PLAN §7): the tab list with its folders and pins, the recently
//! closed tabs, and the sidebar's order. Each part is a step over typed inputs and outputs; the
//! UI carries out what the outputs ask (spawning and ending panes, saving the session).

mod closed;
mod model;
pub mod restore;
#[cfg(test)]
mod restore_tests;
mod sidebar;
mod step;
mod step_close;
mod step_move;
#[cfg(test)]
mod step_props;
#[cfg(test)]
mod step_refusals;
#[cfg(test)]
mod step_tests;
mod step_types;
mod step_viewer;
#[cfg(test)]
mod step_viewer_tests;
mod tabs;
#[cfg(test)]
mod tests;

pub use closed::{ClosedIn, ClosedOut, ClosedState, ClosedTab, KeepClosed, closed_step};
pub use model::Workspace;
pub use sidebar::{SidebarPlan, order, sidebar_plan};
pub use step::workspace_step;
pub use step_types::{RefuseWhy, Spawn, Start, ViewerOpen, WorkspaceIn, WorkspaceOut};
pub use tabs::{NthTab, TabRefusal, TabSel, TabsIn, TabsOut, TabsState, tabs_step};
