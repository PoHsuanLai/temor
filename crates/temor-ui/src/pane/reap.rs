//! How a pane's program is reaped (FOLLOWUPS, temor-pty): hang up, poll for the exit, and kill
//! the process group once the grace period has run out. Pure: the I/O thread asks it what to do
//! after each poll.

use std::time::Duration;

/// How long a program may take to leave after its terminal hangs up.
pub const GRACE: Duration = Duration::from_millis(500);

/// How often the exit is polled while waiting.
pub const POLL: Duration = Duration::from_millis(20);

/// What to do next while waiting for the program to leave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReapStep {
    /// Keep polling for the exit.
    Poll,
    /// The grace period is over: kill the process group.
    Kill,
}

/// The step after `waited` has passed since the hangup.
pub fn reap_step(waited: Duration, grace: Duration) -> ReapStep {
    if waited >= grace {
        ReapStep::Kill
    } else {
        ReapStep::Poll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_gets_the_whole_grace_period_before_a_kill() {
        assert_eq!(reap_step(Duration::ZERO, GRACE), ReapStep::Poll);
        assert_eq!(
            reap_step(GRACE - Duration::from_millis(1), GRACE),
            ReapStep::Poll
        );
        assert_eq!(reap_step(GRACE, GRACE), ReapStep::Kill);
    }
}
