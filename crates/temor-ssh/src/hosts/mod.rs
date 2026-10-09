//! The ⌘T host list (PLAN §9b, D22): `~/.ssh/config` hosts and tailnet machines in one list,
//! each machine once.

mod merge;
mod row;
#[cfg(test)]
mod tests;

pub use merge::merge_hosts;
pub use row::{HostGroup, HostRow};
