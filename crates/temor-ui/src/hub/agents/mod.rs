//! Agents in the window (PLAN §8, P7): the status radar over every pane, the per-pane status
//! sockets, checkpoints and agent worktrees. Pure bookkeeping is in `radar`, `runs`, `trees` and
//! `menu`; git runs on worker threads (`work`, `jobs`); the rest is the hub's side of each.

mod checkpoints;
mod convert;
mod desk;
mod done;
mod jobs;
mod leftovers;
mod loops;
mod menu;
mod observe;
mod radar;
mod runs;
mod state;
#[cfg(test)]
mod tests;
mod trees;
mod work;
mod worktrees;

pub use convert::known_agent;
pub use jobs::Done;
pub use loops::spawn_agent_loops;
pub use menu::{AgentMenu, AgentPick, LeftoverRow, SheetView, Where};
pub use observe::WaitingCount;
pub use radar::{Alert, Changed, Radar};
pub use state::{Agents, Sheet};
pub use work::{
    Made, MakeRequest, Seeded, discard, find_leftovers, listed_as_worktree, make_worktree,
    merge_back, restore, safe_join, seed, snapshot_tree, toplevel,
};
