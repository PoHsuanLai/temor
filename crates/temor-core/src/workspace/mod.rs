//! The workspace's pure model (PLAN §7): the tab list with its folders and pins, the recently
//! closed tabs, and the sidebar's order. Each part is a step over typed inputs and outputs; the
//! UI carries out what the outputs ask (spawning and ending panes, saving the session).

mod sidebar;
mod tabs;
#[cfg(test)]
mod tests;

pub use sidebar::{SidebarPlan, order, sidebar_plan};
pub use tabs::{NthTab, TabRefusal, TabSel, TabsIn, TabsOut, TabsState, tabs_step};
