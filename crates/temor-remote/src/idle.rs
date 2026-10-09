//! How long a detached session is kept (PLAN §9b: until an idle limit, default 7 days).

use temor_wire::link::Monotonic;
use temor_wire::message::IdleLimit;

/// What the sweep does with a detached session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleVerdict {
    /// Leave it running.
    Keep,
    /// End it: it has been detached for the whole limit.
    End,
}

/// The default limit: seven days.
pub const DEFAULT_IDLE: IdleLimit = IdleLimit::After(temor_wire::units::Seconds(7 * 24 * 60 * 60));

/// Whether a session detached at `since` is over `limit` at `now`.
pub fn idle_step(limit: IdleLimit, since: Monotonic, now: Monotonic) -> IdleVerdict {
    match limit {
        IdleLimit::Never => IdleVerdict::Keep,
        IdleLimit::After(seconds) => {
            let kept = now.0.saturating_sub(since.0);
            if kept >= seconds.0.saturating_mul(1000) {
                IdleVerdict::End
            } else {
                IdleVerdict::Keep
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::units::Seconds;

    #[test]
    fn the_limit_is_inclusive_and_never_means_never() {
        let after = IdleLimit::After(Seconds(2));
        let cases = [
            (after, 1_000, 2_999, IdleVerdict::Keep),
            (after, 1_000, 3_000, IdleVerdict::End),
            (after, 5_000, 1_000, IdleVerdict::Keep),
            (IdleLimit::Never, 0, u64::MAX, IdleVerdict::Keep),
        ];
        for (limit, since, now, want) in cases {
            assert_eq!(idle_step(limit, Monotonic(since), Monotonic(now)), want);
        }
    }
}
