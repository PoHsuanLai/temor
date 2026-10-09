//! The OS-specific file operations, behind one `Root` type. Linux resolves every path from a
//! root directory descriptor with `openat2`, refusing symlinks and anything beneath the root's
//! way out. Other platforms refuse with `Unsupported` until they have an equivalent.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub use linux::{Root, sync_folder};
#[cfg(not(target_os = "linux"))]
pub use unsupported::{Root, sync_folder};
