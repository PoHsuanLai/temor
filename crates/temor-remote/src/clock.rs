//! Time, injected. The daemon never reads a clock itself: it asks the [`Clock`] it was given, so
//! tests move time by hand.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use temor_wire::link::{Millis, Monotonic};

/// A monotonic clock. `Send` and `Sync`: the daemon's threads share one.
pub trait Clock: Send + Sync + fmt::Debug {
    /// Milliseconds since an origin the clock picked; never goes backwards.
    fn now(&self) -> Monotonic;
}

/// The real clock: milliseconds since it was made.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// A clock whose origin is now.
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
    fn now(&self) -> Monotonic {
        let millis = u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        Monotonic(millis)
    }
}

/// A clock that moves only when told to, for tests. `Send` and `Sync`.
#[derive(Debug, Default)]
pub struct ManualClock {
    now: AtomicU64,
}

impl ManualClock {
    /// A clock at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves the clock forward.
    pub fn advance(&self, by: Millis) {
        self.now.fetch_add(by.0, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Monotonic {
        Monotonic(self.now.load(Ordering::SeqCst))
    }
}
