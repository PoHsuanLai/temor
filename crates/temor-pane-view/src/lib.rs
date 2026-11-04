//! How temor's window shows a pane (PLAN §7): the pane's session as the window holds it (drawn,
//! read, found in, pointed at, typed into), the keys that reach it, the painter that puts its
//! frames on screen, and the words it says. The window (`temor-ui`) holds these sessions and lays
//! them out; nothing here knows the window's hub or views.

pub mod error;
pub mod input;
pub mod paint;
pub mod pane;
pub mod platform;
pub mod session;
pub mod words;
