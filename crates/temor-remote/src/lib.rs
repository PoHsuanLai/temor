//! temor's helper for ssh hosts (PLAN §9b, Tier B; D18): the session daemon that runs on the
//! host, and the client side the window uses to talk to it.
//!
//! - **Sessions** ([`Daemon`], feature `ghostty`): the daemon owns each session's pseudo-terminal
//!   and keeps its screen in libghostty-vt. A dropped connection detaches the session; attaching
//!   again sends a snapshot of the screen, then the live output, so the exact screen, scrollback
//!   and modes come back.
//! - **Link**: one connection over the ssh channel's stdio, spoken in `temor-wire` frames. The
//!   version is agreed in the greeting; the reconnect machine is `temor_wire::link::link_step`,
//!   which the window drives.
//! - **Ports** ([`ports`]): the listening TCP ports of the sessions' process trees, from `/proc`.
//! - **Files** ([`files`]): list, stat, read, write and watch, confined to the folders the client
//!   was given and resolved with `openat2` beneath them, never through a symlink.
//! - **Facts** ([`facts`]): git's text and the version files of a session's folder, for the
//!   window's chips (a request of its own, the `Facts` capability).
//! - **Client** ([`Client`]): connect over a pair of streams, list, open, attach and detach
//!   sessions, move files, and the actions to offer for a port ([`port_actions`]).
//!
//! The daemon needs libghostty (feature `ghostty`, on by default); without it the pure parts
//! (ports, files, the client) still build.
//!
//! This crate is the app layer's (L3): it starts threads, and takes its clock, its `/proc` root
//! and its shell from the caller. Everything that differs by operating system sits in
//! `platform`, with stubs that refuse on systems other than Linux.
//!
//! # Example
//!
//! Which actions a listening port gets on a tailnet host: open it directly first, then forward.
//!
//! ```
//! use temor_remote::{HostReach, PortAction, port_actions};
//! use temor_wire::units::Port;
//!
//! let reach = HostReach::Tailnet { name: "box.tail1234.ts.net".to_string() };
//! let actions = port_actions(&reach, Port(3000));
//! assert_eq!(actions.last(), Some(&PortAction::Forward { port: Port(3000) }));
//! assert!(matches!(actions.first(), Some(PortAction::OpenAt { .. })));
//! ```

mod client;
mod clock;
#[cfg(feature = "ghostty")]
mod daemon;
mod error;
pub mod facts;
pub mod files;
pub mod host;
mod idle;
mod platform;
pub mod ports;
mod protocol;
mod sync;
mod transport;

pub use client::{Attached, Client, ClientEvent, HostReach, PortAction, ReadChunk, port_actions};
pub use clock::{Clock, ManualClock, SystemClock};
#[cfg(feature = "ghostty")]
pub use daemon::{Daemon, DaemonConfig, Outgoing};
pub use error::Error;
pub use idle::{DEFAULT_IDLE, IdleVerdict, idle_step};
pub use protocol::{DAEMON_CAPS, PROTOCOL, hello};
pub use transport::{FrameReader, FrameWriter};
