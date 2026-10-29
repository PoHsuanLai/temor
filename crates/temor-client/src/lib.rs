//! temor's data as values that are already read, and kept read (PLAN §2b, P24).
//!
//! A caller asks a [`Client`] for what it needs as a [`Live`] value and never waits for it: the
//! client reads it on threads of its own, and again whenever the store says a commit touched what
//! it reads ([`temor_store::changes`]), whoever made the commit, this process or another. So a
//! window never waits on the store, and nothing has to remember to read again after a write.
//!
//! Facts, not views: a query names sessions, commands, places. What a sidebar or a page makes of
//! them is the window's. Ported from mailo's `mail-client`, so the two apps share one shape.

// temor's pure rules and their vocabulary (`temor-core`), for the window, which names them through
// here and no other way (PLAN §2b, the fence; decided 2026-10-28).
pub use temor_core as model;

pub mod act;
mod client;
pub mod desktop;
mod error;
mod external;
pub mod facts;
pub mod icons;
pub mod keep;
mod live;
pub mod plugins;
pub mod query;
pub mod remote;
pub mod shelf;
pub mod space;
pub mod spaces;
pub mod ssh;
pub mod viewer_host;

pub use act::Command;
pub use client::{Client, ClientConfig};
pub use error::ClientError;
pub use live::{Live, Query, Snapshot};
pub use temor_store::{Generation, Shelf, SqliteStore, Topics};
