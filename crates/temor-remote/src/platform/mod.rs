//! Everything that differs by operating system, behind one set of names. Linux has the real
//! implementation: `openat2` for confined file access, `/proc` for ports, and unix sockets for
//! the daemon. Other systems get stubs that refuse with `Error::Unsupported`.

mod model;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

pub use model::RawStat;

#[cfg(target_os = "linux")]
pub use linux::{
    Beneath, Half, Listener, connect, ensure_private_dir, listening_ports, spawn_detached,
};
#[cfg(not(target_os = "linux"))]
pub use unsupported::{
    Beneath, Half, Listener, connect, ensure_private_dir, listening_ports, spawn_detached,
};
