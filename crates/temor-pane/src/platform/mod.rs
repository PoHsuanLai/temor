//! Everything that differs by OS, and the only place `cfg(target_os)` appears in this crate.
//!
//! - Linux: the private socket directory (0700, ours) and the peer's user id from `SO_PEERCRED`.
//! - Other systems: stubs that refuse. TODO(platform:macos) and TODO(platform:windows) mark where
//!   the owner's machines take over.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::{ensure_private_dir, own_uid, peer_uid};

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::{ensure_private_dir, own_uid, peer_uid};
