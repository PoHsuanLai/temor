//! The pane engine: a program on a pty, its terminal under one lock, and the I/O loop that joins
//! them. The caller owns the threads: [`PaneRunner::run`] blocks until the program is reaped.

mod event;
mod fed;
mod fed_io;
mod handle;
mod io;
mod outbox;
mod reap;
mod sink;
mod spawn;
mod term;
mod watch;
mod xfer;

pub use event::{Hold, PaneEvent};
pub use fed::{FedSpec, Feeder, PaneLink, start_fed};
pub use handle::PaneHandle;
pub use io::PaneRunner;
pub use sink::EventSink;
pub use spawn::{PaneSpec, start, start_restored};
pub use term::{Output, Term};
pub use xfer::{OfferId, OfferKind, TransferEnd, TransferEvent, TransferSetup};
