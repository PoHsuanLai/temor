//! The pane engine: a program on a pty, its terminal under one lock, and the I/O loop that joins
//! them. The caller owns the threads: [`PaneRunner::run`] blocks until the program is reaped.

mod event;
mod handle;
mod io;
mod outbox;
mod reap;
mod sink;
mod spawn;
mod term;
mod watch;

pub use event::{Hold, PaneEvent};
pub use handle::PaneHandle;
pub use io::PaneRunner;
pub use sink::EventSink;
pub use spawn::{PaneSpec, start, start_restored};
pub use term::{Output, Term};
