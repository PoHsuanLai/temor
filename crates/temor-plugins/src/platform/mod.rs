//! Running a plugin is Unix only (bayonet starts a process group of its own and polls the pipe).
//! The pure half of the crate builds everywhere; on a system without the Unix half every request
//! answers [`PluginError::Unsupported`](crate::PluginError::Unsupported).
//!
//! TODO(platform:windows): job objects and overlapped pipes behind this edge, once bayonet's run
//! half has them.

#[cfg(not(unix))]
mod other;
#[cfg(unix)]
mod unix;

#[cfg(not(unix))]
pub use other::Launcher;
#[cfg(unix)]
pub use unix::Launcher;
