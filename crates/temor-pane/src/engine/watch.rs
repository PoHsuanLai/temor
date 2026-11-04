//! Watching the pane's foreground process: the I/O thread reads its name and command line about
//! once a second (`tcgetpgrp` and `/proc`, through temor-pty) and tells the window only when
//! either changes. The radar uses it to know a known agent runs in the pane.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use temor_pty::{ProcRoot, Pty};

use super::event::{PaneEvent, SeenForeground};

/// How often the foreground process is looked at.
const EVERY: Duration = Duration::from_secs(1);

/// The last foreground sent, and when the process was last looked at.
#[derive(Debug)]
pub(super) struct ForegroundWatch {
    last: Option<Option<SeenForeground>>,
    checked: Option<Instant>,
}

impl ForegroundWatch {
    pub(super) fn new() -> Self {
        Self {
            last: None,
            checked: None,
        }
    }

    /// The event to send when, at `now`, it is time to look and the name or command line
    /// changed.
    pub(super) fn poll(&mut self, pty: &Pty, now: Instant) -> Option<PaneEvent> {
        if !due(self.checked, now) {
            return None;
        }
        self.checked = Some(now);
        let seen = pty
            .foreground(&ProcRoot(PathBuf::from("/proc")))
            .ok()
            .and_then(|fg| {
                fg.name.map(|name| SeenForeground {
                    name,
                    command: fg.command,
                })
            });
        let event = changed(self.last.as_ref(), &seen);
        self.last = Some(seen);
        event
    }
}

/// Whether it is time to look again at `now`, last looked at `checked`.
fn due(checked: Option<Instant>, now: Instant) -> bool {
    checked.is_none_or(|at| now.saturating_duration_since(at) >= EVERY)
}

/// What to tell the window when `seen` is in the foreground after `last` was sent: nothing when
/// it is the same.
fn changed(
    last: Option<&Option<SeenForeground>>,
    seen: &Option<SeenForeground>,
) -> Option<PaneEvent> {
    (last != Some(seen)).then(|| PaneEvent::Foreground(seen.clone()))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{EVERY, changed, due};
    use crate::engine::event::{PaneEvent, SeenForeground};

    #[test]
    fn the_foreground_is_looked_at_once_a_second() {
        let at = Instant::now();
        assert!(due(None, at));
        assert!(!due(Some(at), at + EVERY - Duration::from_millis(1)));
        assert!(due(Some(at), at + EVERY));
    }

    #[test]
    fn the_window_hears_the_foreground_only_when_it_changes() {
        let vim = Some(SeenForeground {
            name: "vim".into(),
            command: None,
        });
        let told = |last, seen: &Option<SeenForeground>| match changed(last, seen) {
            Some(PaneEvent::Foreground(now)) => Some(now),
            _ => None,
        };
        assert_eq!(told(None, &None), Some(None));
        assert_eq!(told(Some(&None), &None), None);
        assert_eq!(told(Some(&None), &vim), Some(vim.clone()));
        assert_eq!(told(Some(&vim), &vim), None);
    }
}
