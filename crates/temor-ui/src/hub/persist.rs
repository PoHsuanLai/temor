//! Saving the session (PLAN §4b: never in the render path). A change marks the session dirty;
//! the window polls on a timer and the pace lets one write through per interval. The last
//! write and the snapshots of the live panes happen when the hub is dropped, at quit.

use std::time::Instant;

use super::model::Hub;
use crate::edge::{Pace, save_with_hosts, sidecar};
use crate::session::Session;

impl Hub {
    /// Writes the session if a change is waiting and the pace allows it. Returns how long to
    /// wait before asking again, when a write is waiting for its turn.
    pub fn poll_save(&mut self, now: Instant) -> Option<std::time::Duration> {
        match self.pace.poll(now) {
            Pace::Idle => None,
            Pace::WaitFor(wait) => Some(wait),
            Pace::WriteNow => {
                self.write_session();
                self.pace.wrote(now);
                None
            }
        }
    }

    fn write_session(&mut self) {
        let Some(dir) = &self.state_dir else { return };
        if let Err(err) = save_with_hosts(dir, &self.ws, &self.ssh.hosts_file()) {
            self.notices.push(err.to_string());
        }
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        let Some(dir) = self.state_dir.clone() else {
            return;
        };
        let _ = save_with_hosts(&dir, &self.ws, &self.ssh.hosts_file());
        // Ids restart from the live maximum, so what an earlier quit left must not outlive this one.
        sidecar::clear(&dir);
        for (pane, slot) in &self.slots {
            let snapshot = slot.borrow().session.as_ref().and_then(Session::snapshot);
            if let Some(snapshot) = snapshot {
                let _ = sidecar::write(&dir, *pane, &snapshot);
            }
        }
    }
}

/// How often the window asks whether a change is waiting to be written.
const SAVE_TICK: std::time::Duration = std::time::Duration::from_millis(500);

/// Starts the timer that writes the session for as long as the window lives.
pub fn spawn_save_loop(hub: super::model::HubRef) {
    dioxus::prelude::spawn(async move {
        loop {
            ds::base::time::clock::sleep(SAVE_TICK).await;
            let _ = hub.0.borrow_mut().poll_save(Instant::now());
        }
    });
}
