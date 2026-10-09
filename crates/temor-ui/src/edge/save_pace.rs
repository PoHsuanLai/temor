//! When the session is written (PLAN §4b: never in the render path). A change marks the state
//! dirty; the window polls on its own timer and writes at most once per interval, and once more
//! on quit. Time is passed in, so the rule is testable without a clock.

use std::time::{Duration, Instant};

/// What the window should do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    /// Nothing changed since the last write.
    Idle,
    /// Something changed, and the interval has passed: write now.
    WriteNow,
    /// Something changed; write once this long has passed.
    WaitFor(Duration),
}

/// The write pacing of the session file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePace {
    interval: Duration,
    last_write: Option<Instant>,
    dirty: bool,
}

impl SavePace {
    /// A pace that writes at most once per `interval`.
    pub fn new(interval: Duration) -> SavePace {
        SavePace {
            interval,
            last_write: None,
            dirty: false,
        }
    }

    /// The state changed.
    pub fn changed(&mut self) {
        self.dirty = true;
    }

    /// What to do at `now`.
    pub fn poll(&self, now: Instant) -> Pace {
        if !self.dirty {
            return Pace::Idle;
        }
        match self.last_write {
            None => Pace::WriteNow,
            Some(last) => {
                let since = now.saturating_duration_since(last);
                if since >= self.interval {
                    Pace::WriteNow
                } else {
                    Pace::WaitFor(self.interval - since)
                }
            }
        }
    }

    /// A write happened at `now`.
    pub fn wrote(&mut self, now: Instant) {
        self.dirty = false;
        self.last_write = Some(now);
    }

    /// Whether a write is owed at quit (a change not yet written).
    pub fn owed(&self) -> bool {
        self.dirty
    }
}
