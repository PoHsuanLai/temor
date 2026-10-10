//! The client side, for the window (PLAN §9b): connect over a pair of streams (the ssh process's
//! stdio), list, open, attach and detach sessions, ask for ports and files, and move files in
//! and out. The window feeds `Output` events to the terminal it restored from the attach
//! snapshot; the reconnect machine is `temor_wire::link::link_step`, driven by the window.

mod calls;
mod connect;
mod events;
mod facts;
mod offer;
mod pending;
mod stream;
mod transfer;

pub use calls::{Attached, ReadChunk};
pub use connect::Client;
pub use events::ClientEvent;
pub use offer::{HostReach, PortAction, port_actions};
