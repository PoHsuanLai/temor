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

    /// The event to send when it is time to look and the name or command line changed.
    pub(super) fn poll(&mut self, pty: &Pty) -> Option<PaneEvent> {
        let due = self.checked.is_none_or(|at| at.elapsed() >= EVERY);
        if !due {
            return None;
        }
        self.checked = Some(Instant::now());
        let seen = pty
            .foreground(&ProcRoot(PathBuf::from("/proc")))
            .ok()
            .and_then(|fg| {
                fg.name.map(|name| SeenForeground {
                    name,
                    command: fg.command,
                })
            });
        if self.last.as_ref() == Some(&seen) {
            return None;
        }
        self.last = Some(seen.clone());
        Some(PaneEvent::Foreground(seen))
    }
}
