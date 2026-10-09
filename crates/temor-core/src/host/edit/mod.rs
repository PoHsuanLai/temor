//! Edit a host's file locally and write it back (§9b "Remote files"). The file is fetched to a
//! local copy and opened in the local editor or viewer; when the copy is saved, the host's file
//! is checked first (modified time and size) and overwritten only if it is as it was fetched.
//!
//! `edit_step` decides; the app runs the edge calls it names (temor-ssh's `RemoteFiles`) and
//! `watch_step` turns polls of the local copy into saves.

mod model;
mod step;
#[cfg(test)]
mod tests;
mod watch;

pub use model::{Edit, EditFault, EditIn, EditOut, Found, Queued, Resolution};
pub use step::edit_step;
pub use watch::{LocalStamp, Watch, watch_step};
