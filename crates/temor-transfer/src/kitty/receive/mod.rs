//! The receive machine: `ReceiveSession` takes frames and the person's consent, and returns the
//! replies and file writes to carry out.

mod model;
mod step;

pub use model::{Consent, Effect, EndWhy, Input, ReceiveSession, SavedFile, WriteOp};
