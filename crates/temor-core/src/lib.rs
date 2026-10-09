//! temor's pure core (PLAN.md §2–§4, §7, §8.3): the vocabulary, the split tree, every step
//! machine, the command-bar ranking and proposal sanitising. No I/O, no threads, no UI and no
//! foreign code: the other crates feed these functions values and carry out what they return.
//! The one exception is `worktree::ProcessGit`, the edge that starts git from a binary path the
//! caller injects; everything else reaches git through the `Git` trait.

pub mod agent;
pub mod bar;
pub mod block;
pub mod checkpoint;
pub mod error;
pub mod host;
pub mod key;
pub mod notify;
pub mod pane;
pub mod presence;
pub mod proposal;
pub mod quick;
pub mod remote;
pub mod shelf;
pub mod space;
pub mod split;
pub mod transfer;
pub mod vocab;
pub mod workspace;
pub mod worktree;
