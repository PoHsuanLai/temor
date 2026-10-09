//! The workspace's pure model (PLAN §7): the tab list with its folders and pins, the recently
//! closed tabs, and the sidebar's order. Each part is a step over typed inputs and outputs; the
//! UI carries out what the outputs ask (spawning and ending panes, saving the session).

mod closed;
mod model;
mod sidebar;
mod step;
#[cfg(test)]
mod step_tests;
mod tabs;
#[cfg(test)]
mod tests;

pub use closed::{ClosedIn, ClosedOut, ClosedState, ClosedTab, KeepClosed, closed_step};
pub use model::Workspace;
pub use sidebar::{SidebarPlan, order, sidebar_plan};
pub use step::{RefuseWhy, Spawn, Start, WorkspaceIn, WorkspaceOut, workspace_step};
pub use tabs::{NthTab, TabRefusal, TabSel, TabsIn, TabsOut, TabsState, tabs_step};
