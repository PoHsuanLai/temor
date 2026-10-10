//! The file service (PLAN §9b, "Files"): list, stat, read, write and watch, for the client's
//! open-and-write-back and its live viewers. Every path is confined to the folders the client was
//! given (see [`Scope`]) and resolved beneath them without crossing a link.

mod part;
mod scope;
mod service;
mod stamp;
mod sweep;
mod watch;
mod write;

pub use scope::{Resolved, Scope};
pub use service::FileService;
pub use stamp::sha256;
pub use sweep::{STALE_AFTER, is_stale_stage};
pub use watch::{Observed, WatchTable, watch_step};
pub use write::{STAGE_PREFIX, WriteDecision, stage_name, write_decision};
