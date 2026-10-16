//! Cancelling a request: the next keystroke makes the last one stale.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// What a running request is told between messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancel {
    /// Carry on.
    Go,
    /// Stop now.
    Stop,
}

/// The newest keystroke. Cloning shares the count, so a worker started for one keystroke can see
/// that a newer one has come.
#[derive(Debug, Clone, Default)]
pub struct Generations(Arc<AtomicU64>);

impl Generations {
    /// A counter that has seen no keystroke.
    pub fn new() -> Generations {
        Generations::default()
    }

    /// A keystroke: the number of the one that is now newest.
    pub fn begin(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// The newest keystroke so far (zero before any).
    pub fn newest(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    /// Whether a request started at `mine` should stop.
    pub fn check(&self, mine: u64) -> Cancel {
        match self.newest() == mine {
            true => Cancel::Go,
            false => Cancel::Stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_keystroke_cancels_the_older_ones_only() {
        let keys = Generations::new();
        let first = keys.begin();
        let shared = keys.clone();
        assert_eq!(keys.check(first), Cancel::Go);
        let second = shared.begin();
        assert_eq!((first, second), (1, 2));
        assert_eq!(keys.check(first), Cancel::Stop);
        assert_eq!(keys.check(second), Cancel::Go);
    }
}
