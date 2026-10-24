//! Saving the session (PLAN §4b: never in the render path). A change marks the session dirty;
//! the window polls on a timer and the pace lets one write through per interval. The last
//! write and the snapshots of the live panes happen when the hub is dropped, at quit.

use std::collections::BTreeSet;
use std::time::Instant;

use ds::components::app::spaces::SpaceId;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::leave_out;

use super::model::Hub;
use crate::edge::{Pace, checkpoint, save_session_for, save_spaces, save_with_hosts, sidecar};
use crate::session::Session;

impl Hub {
    /// Writes the session if a change is waiting and the pace allows it. Returns how long to
    /// wait before asking again, when a write is waiting for its turn.
    pub fn poll_save(&mut self, now: Instant) -> Option<std::time::Duration> {
        self.keep_checkpoints();
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
        // Quitting from the system's menu ends the process without dropping the hub, so what a
        // next launch needs from each pane (its shell's state, its daemon session) is kept now.
        self.write_checkpoints(&dir);
    }

    /// Writes the checkpoint of each pane whose state changed since it was last written, so a
    /// quit that never reaches the hub's drop still leaves the shells' state for the next launch.
    fn keep_checkpoints(&mut self) {
        let Some(dir) = self.state_dir.clone() else {
            return;
        };
        let now: Vec<_> = self
            .slots
            .iter()
            .filter(|(pane, _)| !self.is_quick(**pane) && !self.run.panes.contains_key(pane))
            .filter_map(|(pane, slot)| {
                let mut kept = slot.borrow().session.as_ref().map(Session::checkpoint)?;
                self.keep_local(*pane, &mut kept);
                Some((*pane, kept))
            })
            .collect();
        for (pane, kept) in now {
            if self.on_disk.get(&pane) != Some(&kept)
                && checkpoint::write(&dir, pane, &kept).is_ok()
            {
                self.on_disk.insert(pane, kept);
            }
        }
    }

    /// Writes each pane's checkpoint, with the daemon session it has.
    fn write_checkpoints(&self, dir: &std::path::Path) {
        for (pane, slot) in &self.slots {
            // The quick terminal is a throwaway: kept while temor runs, never saved; nor is a run tab.
            if self.is_quick(*pane) || self.run.panes.contains_key(pane) {
                continue;
            }
            let kept = slot.borrow().session.as_ref().map(Session::checkpoint);
            if let Some(mut kept) = kept {
                self.keep_local(*pane, &mut kept);
                let _ = checkpoint::write(dir, *pane, &kept);
            }
        }
    }

    /// Writes every Space's session and the Spaces file; what could not be written, as text.
    fn save_all(&self, dir: &std::path::Path) -> Vec<String> {
        // A tab that ran a program (`temor -e`) is not brought back: it was that launch's.
        let runs: BTreeSet<PaneId> = self.run.panes.keys().copied().collect();
        let sessions = self
            .book
            .parked
            .iter()
            .map(|(id, ws)| (*id, ws))
            .chain([(self.book.active, &self.ws)])
            .map(|(id, ws)| (id, leave_out(ws, &runs)));
        let mut failures: Vec<String> = sessions
            .filter_map(|(id, ws)| match id {
                // The first Space's file also keeps the hosts (answers and panes on hosts).
                SpaceId(0) => save_with_hosts(dir, &ws, &self.hosts_file()).err(),
                _ => save_session_for(dir, id, &ws).err(),
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
        // The links go, the host sessions stay: quitting detaches (D18).
        self.remote_quit();
        // Ids restart from the live maximum, so what an earlier quit left must not outlive this one.
        sidecar::clear(&dir);
        checkpoint::clear(&dir);
        crate::edge::replay::clear(&dir);
        self.write_checkpoints(&dir);
        for (pane, slot) in &self.slots {
            if self.is_quick(*pane) || self.run.panes.contains_key(pane) {
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
