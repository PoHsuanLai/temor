//! Linux: confined file access, `/proc` and unix sockets.

mod beneath;
mod procfs;
mod socket;
mod spawn;
mod stat;

pub use beneath::Beneath;
pub use procfs::listening_ports;
pub use socket::{Half, Listener, connect, ensure_private_dir};
pub use spawn::spawn_detached;
