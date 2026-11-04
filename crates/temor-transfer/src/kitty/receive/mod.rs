//! The receive machine: `ReceiveSession` takes frames, the person's consent, aborts and write
//! failures, and returns the replies and file writes to carry out.

mod files;
mod helpers;
mod links;
mod model;
mod step;

pub use model::{Effect, Input, LinkPolicy, ReceiveSession, SavedFile, TransferEnd, WriteOp};
