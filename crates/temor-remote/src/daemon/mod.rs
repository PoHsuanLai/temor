//! The session daemon (PLAN §9b, "Durable sessions"): one per person on the host. It owns each
//! session's pseudo-terminal and keeps its screen in libghostty, so a dropped connection loses
//! nothing: reattaching sends a snapshot of the screen, then the live output.
//!
//! [`Daemon::serve`] runs one connection (the ssh channel's stdio, or the daemon's local socket)
//! and returns when it ends; the sessions go on. [`Daemon::tick`] does the periodic work (idle
//! sweep, port report, watches) and is driven by the caller's timer, so tests drive it by hand.

mod config;
mod core;
mod cwd;
mod dispatch;
mod exit;
mod input;
mod link;
mod pump;
mod registry;
mod serve;
mod session;
mod sinks;
mod tick;

pub use config::DaemonConfig;
pub use core::Daemon;
pub use link::Outgoing;
