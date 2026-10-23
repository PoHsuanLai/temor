//! Everything that differs by operating system, behind one set of names. Linux has the real
//! implementation: `openat2` for confined file access and `/proc` for ports. Any unix has the
//! unix sockets for the daemon (so a local daemon runs on macOS too). What a system lacks is a
//! stub that refuses with `Error::Unsupported`.

mod model;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(unix)]
mod unix;
#[cfg(not(all(unix, target_os = "linux")))]
mod unsupported;

pub use model::RawStat;

#[cfg(target_os = "linux")]
pub use linux::{Beneath, listening_ports};
#[cfg(unix)]
pub use unix::{Half, Listener, connect, ensure_private_dir, spawn_detached};
#[cfg(not(target_os = "linux"))]
pub use unsupported::{Beneath, listening_ports};
#[cfg(not(unix))]
pub use unsupported::{Half, Listener, connect, ensure_private_dir, spawn_detached};
