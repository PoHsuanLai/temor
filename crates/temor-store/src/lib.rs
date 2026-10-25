//! Where temor's data is kept (PLAN §2b, P23): SQLite in WAL mode, with FTS5 over the commands.
//!
//! Does disk I/O and nothing else: no clock (every time is handed in), no sockets, no UI. The
//! schema is `migrations/0001_initial.sql`. What may be written (never a secret, never a command
//! typed after a space) and how long history is kept are the client's rules; the store keeps what
//! it is given and forgets what it is told to.

pub mod changes;
pub mod error;
mod migrate;
mod sqlite;
mod tables;

pub use changes::{Committed, Generation, Topics};
pub use error::StoreError;
pub use migrate::EXPECTED_VERSION;
pub use sqlite::{SqliteStore, Writer};
pub use tables::{Found, Shelf};
