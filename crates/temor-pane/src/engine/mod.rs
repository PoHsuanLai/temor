//! The pane engine: a program on a pty, its terminal under one lock, and the I/O loop that joins
//! them. The caller owns the threads: [`PaneRunner::run`] blocks until the program is reaped.

mod clipboard;
mod event;
mod fed;
mod fed_io;
mod handle;
mod hold;
mod io;
mod outbox;
mod poison;
mod reap;
mod sink;
mod spawn;
mod tap;
mod term;
mod watch;
mod xfer;

pub use clipboard::{ClipboardMirror, ReadPolicy};
pub use event::{Hold, PaneEvent, SeenForeground};
pub use fed::{FedSpec, Feeder, PaneLink, start_fed};
pub use handle::PaneHandle;
pub use hold::PaintSource;
pub use io::PaneRunner;
pub use sink::EventSink;
pub(crate) use spawn::pty_size;
pub use spawn::{PaneSpec, start, start_restored};
pub use tap::ByteTap;
pub use term::{Output, Term};
pub use xfer::{OfferId, OfferKind, TransferEnd, TransferEvent, TransferSetup};
