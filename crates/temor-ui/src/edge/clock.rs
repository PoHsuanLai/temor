//! The clock the session reads for block times and notice gaps: whole milliseconds since the
//! session started (PLAN §3, `Stamp`). Injected, so tests run on a fake clock.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use ds_core::time::stamp::Stamp;

/// Where "now" comes from.
pub trait Clock {
    fn now(&self) -> Stamp;
}

/// The monotonic clock, counted from when it was made.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Stamp {
        Stamp(u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX))
    }
}

/// A clock the test moves by hand. Clones share the time.
#[derive(Debug, Clone, Default)]
pub struct FakeClock {
    now: Rc<Cell<u64>>,
}

impl FakeClock {
    pub fn at(ms: u64) -> Self {
        Self {
            now: Rc::new(Cell::new(ms)),
        }
    }

    /// Moves the time forward by `ms`.
    pub fn advance(&self, ms: u64) {
        self.now.set(self.now.get() + ms);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Stamp {
        Stamp(self.now.get())
    }
}
