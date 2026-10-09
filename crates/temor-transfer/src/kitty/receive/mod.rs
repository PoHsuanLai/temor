//! The receive machine: `ReceiveSession` takes frames, the person's consent, aborts and write
//! failures, and returns the replies and file writes to carry out.

mod files;
mod helpers;
mod model;
mod step;

pub use model::{Effect, EndWhy, Input, ReceiveSession, SavedFile, WriteOp};
