//! Backoff: how long to wait before each dial, and when to stop.

use super::model::Millis;

/// A dial attempt, counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Attempt(pub u32);

impl Attempt {
    /// The attempt after this one.
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// The retry schedule. Deterministic: no jitter, so the same failures always give the same
/// timings, and tests can state them.
///
/// Attempt 1 is immediate. Attempt `n` (from 2) waits `base * 2^(n-2)`, capped at `cap`. The link
/// gives up once `attempts` dials have failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// The first real wait, after the immediate first attempt.
    pub base: Millis,
    /// The longest wait.
    pub cap: Millis,
    /// How many dials in all before giving up.
    pub attempts: Attempt,
}

impl RetryPolicy {
    /// One second to start, doubling to a one-minute cap, and 30 attempts (about half an hour of
    /// waiting), enough to ride out a laptop's lid being closed.
    pub const DEFAULT: Self = Self {
        base: Millis(1_000),
        cap: Millis(60_000),
        attempts: Attempt(30),
    };

    /// The wait before `attempt`.
    pub fn delay(&self, attempt: Attempt) -> Millis {
        if attempt.0 <= 1 {
            return Millis(0);
        }
        let doublings = (attempt.0 - 2).min(63);
        let raw = self.base.0.saturating_mul(1u64 << doublings);
        Millis(raw.min(self.cap.0))
    }
}

/// How the caller probes an open link: ping every `every`, and call the link stalled if no
/// answer comes within `within`. The caller runs the timers; [`LinkEvent::PingTimedOut`] is what
/// it reports back.
///
/// [`LinkEvent::PingTimedOut`]: super::LinkEvent::PingTimedOut
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingPolicy {
    /// The time between pings while the link is idle.
    pub every: Millis,
    /// How long to wait for the answer.
    pub within: Millis,
}

impl PingPolicy {
    /// Every 15 seconds, giving up on an answer after 10: a stalled link is noticed in under
    /// half a minute and a slow but live one is not cut.
    pub const DEFAULT: Self = Self {
        every: Millis(15_000),
        within: Millis(10_000),
    };
}
