//! Moments and lengths of time, each a newtype so a wall-clock time is never mistaken for a
//! reading of a clock that only counts up, nor either of them for how long something lasts.

use serde::{Deserialize, Serialize};

/// A moment on the wall clock, in milliseconds since the Unix epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct UnixMillis(pub i64);

/// A length of time in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Millis(pub u64);

/// A point on the caller's monotonic clock, in milliseconds since whatever origin the caller
/// picked: only differences between two of them mean anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Monotonic(pub u64);

impl Monotonic {
    /// Milliseconds from `earlier` to `self`; zero if the clock stepped back.
    pub fn since(self, earlier: Monotonic) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}
