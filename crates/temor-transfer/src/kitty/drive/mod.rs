//! Blocking drivers that run the kitty machines over a [`Link`], such as the controlling
//! terminal of a program. They are what `temor save` and `temor get` use. The library starts
//! no threads: the caller's thread waits on the link.

mod receive;
mod send;

use std::io;
use std::time::Duration;

pub use receive::drive_receive;
pub use send::drive_send;

/// A two-way line to the other side of a transfer. `Send`-ness is the implementor's to state.
pub trait Link {
    /// Writes all of `bytes`.
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
    /// Waits up to `timeout` for bytes. `Ok(None)` means nothing came in time.
    fn read(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>>;
}

/// How long to wait for the other side. The first answer may wait for a person to say yes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waits {
    pub first: Duration,
    pub next: Duration,
}
