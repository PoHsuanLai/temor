//! The window's side of the desktop (PLAN §4b, §8.4, D22): the port the desktop services call
//! and the window answers, and the Tailscale computers as the window reads them.

pub mod feed;
pub mod port;

// The desktop services' own names, for the window's side of the port, until it is the client's.
pub use temor_desktop::*;
