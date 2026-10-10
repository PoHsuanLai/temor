//! Transfers in a pane (PLAN §9): what the program sends through OSC 1337 `File=` and kitty's
//! OSC 5113, and what it asks the terminal for. It runs on the pane's I/O thread and never waits:
//! an offer becomes an event, the person's answer comes back as a command, and the files land
//! in a staging folder the window empties into the shelf.

mod event;
mod inline;
mod receive;
mod send;
mod state;
#[cfg(test)]
mod tests;

pub use event::{OfferId, OfferKind, TransferEnd, TransferEvent, TransferSetup};
pub(super) use state::{Reaction, Transfers};
