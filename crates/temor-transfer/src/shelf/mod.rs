//! The downloads shelf: what has been received, newest first, with where it came from. The
//! model is pure; the store reads and writes the index and the files beside it.

mod clock;
mod model;
mod naming;
mod store;

pub use clock::{Clock, SystemClock};
pub use model::{Shelf, ShelfId, ShelfItem, UnixMillis};
pub use naming::{candidate_name, unique_name};
pub use store::ShelfStore;
