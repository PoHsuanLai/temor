//! Watching the pane's foreground process: the I/O thread reads its name about once a second
//! (`tcgetpgrp` and `/proc`, through temor-pty) and tells the window only when it changes. The
//! radar uses it to know a known agent runs in the pane.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use temor_pty::{ProcRoot, Pty};

use super::event::PaneEvent;

/// How often the foreground process is looked at.
const EVERY: Duration = Duration::from_secs(1);

/// The last foreground name sent, and when the process was last looked at.
#[derive(Debug)]
pub(super) struct ForegroundWatch {
    last: Option<Option<String>>,
    checked: Option<Instant>,
}

impl ForegroundWatch {
    pub(super) fn new() -> Self {
        Self {
            last: None,
            checked: None,
        }
    }

    /// The event to send when it is time to look and the name changed.
    pub(super) fn poll(&mut self, pty: &Pty) -> Option<PaneEvent> {
        let due = self.checked.is_none_or(|at| at.elapsed() >= EVERY);
        if !due {
            return None;
        }
        self.checked = Some(Instant::now());
        let name = pty
            .foreground(&ProcRoot(PathBuf::from("/proc")))
            .ok()
            .and_then(|fg| fg.name);
        if self.last.as_ref() == Some(&name) {
            return None;
        }
        self.last = Some(name.clone());
        Some(PaneEvent::Foreground(name))
    }
}
