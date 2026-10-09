//! One terminal pane's engine (PLAN §4b): the terminal and its encoders under one lock, the PTY
//! and its I/O thread, and the events the window hears. The window never touches the PTY.

mod event;
mod handle;
mod io;
mod outbox;
mod reap;
mod spawn;
mod term;

pub use event::{Hold, PaneEvent};
pub use handle::PaneHandle;
pub use reap::{ReapStep, reap_step};
pub use spawn::{PaneSpec, start};
pub use term::{Output, Term};
