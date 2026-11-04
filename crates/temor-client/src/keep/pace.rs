//! When the session is written (PLAN §4b: never in the render path). The window polls on its own
//! timer, saying whether what is kept differs from what was written, and writes at most once per
//! interval, and once more on quit. Time is passed in, so the rule is testable without a clock.

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
}

impl SavePace {
    /// A pace that writes at most once per `interval`.
    pub fn new(interval: Duration) -> SavePace {
        SavePace {
            interval,
            last_write: None,
        }
    }

    /// What to do at `now`, when what is kept differs from what was written (`owed`).
    pub fn poll(&self, now: Instant, owed: bool) -> Pace {
        if !owed {
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
        self.last_write = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_is_written_at_once_then_at_most_once_per_interval() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut pace = SavePace::new(Duration::from_millis(2000));
        assert_eq!(pace.poll(at(0), false), Pace::Idle);
        assert_eq!(pace.poll(at(0), true), Pace::WriteNow);
        pace.wrote(at(0));
        assert_eq!(pace.poll(at(500), false), Pace::Idle);
        assert_eq!(
            pace.poll(at(500), true),
            Pace::WaitFor(Duration::from_millis(1500))
        );
        assert_eq!(pace.poll(at(2000), true), Pace::WriteNow);
    }
}
