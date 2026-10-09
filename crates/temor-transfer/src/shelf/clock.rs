//! The clock is injected. `SystemClock` is the only reader of wall-clock time in this crate and
//! is meant to be built at the app's edge.

use super::model::UnixMillis;

/// Gives the current time to whoever records a receipt.
pub trait Clock {
    fn now(&self) -> UnixMillis;
}

/// The system's wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UnixMillis {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
            });
        UnixMillis::new(millis)
    }
}
