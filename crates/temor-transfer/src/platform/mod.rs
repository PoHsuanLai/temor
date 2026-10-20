//! The OS-specific file operations, behind one `Root` type. Linux and macOS resolve every path from
//! a root directory descriptor, one name at a time, refusing symlinks and anything beneath the
//! root's way out (`openat2` on Linux, `O_NOFOLLOW` on macOS). Other platforms refuse with
//! `Unsupported` until they have an equivalent.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use unix::{Root, sync_folder};
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use unsupported::{Root, sync_folder};
