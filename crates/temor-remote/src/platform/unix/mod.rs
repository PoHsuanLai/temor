//! Any unix: the daemon's local socket and starting it detached.

mod socket;
mod spawn;

pub use socket::{Half, Listener, connect, ensure_private_dir};
pub use spawn::spawn_detached;
