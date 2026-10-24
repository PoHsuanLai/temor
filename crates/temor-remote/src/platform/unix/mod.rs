//! Any unix: the daemon's local socket and starting it detached.

mod socket;
mod spawn;

pub use socket::{Half, Listener, SocketLock, connect, ensure_private_dir, remove_socket};
pub use spawn::spawn_detached;
