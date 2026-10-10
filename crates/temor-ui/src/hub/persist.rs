//! Saving the session (PLAN §4b: never in the render path). A change marks the session dirty;
//! the window polls on a timer and the pace lets one write through per interval. The last
//! write and the snapshots of the live panes happen when the hub is dropped, at quit.

use std::time::Instant;

use ds::components::app::spaces::SpaceId;

use super::model::Hub;
use crate::edge::{Pace, save_session_for, save_spaces, save_with_hosts, sidecar};
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
        let Some(dir) = self.state_dir.clone() else {
            return;
        };
        let failures = self.save_all(&dir);
        self.notices.extend(failures);
    }

    /// Writes every Space's session and the Spaces file; what could not be written, as text.
    fn save_all(&self, dir: &std::path::Path) -> Vec<String> {
        let sessions = self
            .book
            .parked
            .iter()
            .map(|(id, ws)| (*id, ws))
            .chain([(self.book.active, &self.ws)]);
        let mut failures: Vec<String> = sessions
            .filter_map(|(id, ws)| match id {
                // The first Space's file also keeps the hosts (answers and panes on hosts).
                SpaceId(0) => save_with_hosts(dir, ws, &self.ssh.hosts_file()).err(),
                _ => save_session_for(dir, id, ws).err(),
            })
            .map(|err| err.to_string())
            .collect();
        if let Err(err) = save_spaces(dir, &self.spaces_for_file()) {
            failures.push(err.to_string());
        }
        failures
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        let Some(dir) = self.state_dir.clone() else {
            return;
        };
        let _ = self.save_all(&dir);
        // Ids restart from the live maximum, so what an earlier quit left must not outlive this one.
        sidecar::clear(&dir);
        for (pane, slot) in &self.slots {
            // The quick terminal is a throwaway: kept while temor runs, never saved.
            if self.is_quick(*pane) {
                continue;
            }
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
