//! The clock the session reads for block times and notice gaps: whole milliseconds since the
//! session started (PLAN §3, `Stamp`). Injected, so tests run on a fake clock.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use ds_core::time::stamp::Stamp;

/// Where "now" comes from.
pub trait Clock: Send {
    fn now(&self) -> Stamp;

    /// `instant`, a moment already past (when a pane read a mark), on this clock.
    fn at(&self, instant: Instant) -> Stamp;
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
        self.at(Instant::now())
    }

    fn at(&self, instant: Instant) -> Stamp {
        let since = instant.saturating_duration_since(self.origin);
        Stamp(u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
    }
}

/// A clock the test moves by hand. Clones share the time.
#[derive(Debug, Clone, Default)]
pub struct FakeClock {
    now: Arc<AtomicU64>,
}

impl FakeClock {
    pub fn at(ms: u64) -> Self {
        Self {
            now: Arc::new(AtomicU64::new(ms)),
        }
    }

    /// Moves the time forward by `ms`.
    pub fn advance(&self, ms: u64) {
        self.now.fetch_add(ms, Ordering::Relaxed);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Stamp {
        Stamp(self.now.load(Ordering::Relaxed))
    }

    /// The time the test set: a test moves time by hand, never by how long a mark waited.
    fn at(&self, _instant: Instant) -> Stamp {
        self.now()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Clock, SystemClock};

    /// Two marks read 250 ms apart are 250 ms apart however late the window takes them; a moment
    /// before the clock began is its zero.
    #[test]
    fn the_system_clock_stamps_a_past_moment_where_it_happened() {
        let clock = SystemClock::new();
        let start = clock.origin + Duration::from_millis(1_000);
        let end = start + Duration::from_millis(250);
        assert_eq!(clock.at(end).since(clock.at(start)), 250);
        assert_eq!(clock.at(Instant::now() - Duration::from_secs(5)).0, 0);
    }
}
