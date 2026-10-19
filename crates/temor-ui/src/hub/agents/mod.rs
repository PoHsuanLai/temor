//! Agents in the window (PLAN §8, P7): the status radar over every pane, the per-pane status
//! sockets, restore points and agent worktrees. Pure bookkeeping is in `radar`, `points`, `trees` and
//! `menu`; git runs on worker threads (`work`, `jobs`); the rest is the hub's side of each.

mod checkpoints;
mod convert;
mod desk;
mod done;
mod ending;
mod jobs;
mod leftovers;
mod loops;
mod menu;
mod observe;
mod points;
#[cfg(test)]
mod points_tests;
mod radar;
mod state;
mod sweep;
#[cfg(test)]
mod tests;
mod trees;
mod watch;
#[cfg(test)]
mod watch_tests;
mod watching;
mod work;
mod worktrees;

pub use convert::known_agent;
pub use jobs::Done;
pub use loops::spawn_agent_loops;
pub use menu::{AgentMenu, AgentPick, LeftoverRow, SheetView, Where};
pub use observe::WaitingCount;
pub use points::{PointLine, RestoreMenu, WAIT_FOR_AGENT};
pub use radar::{Alert, Changed, Radar};
pub use state::{Agents, Sheet};
pub use work::{
    Made, MakeRequest, discard, find_leftovers, listed_as_worktree, make_worktree, merge_back,
    safe_join, snapshot_tree, sweep_snapshots, toplevel, unused_worktrees,
};
