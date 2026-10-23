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

    /// What is kept of each pane that outlives the window: its checkpoint (with the daemon
    /// session it has) and, with `Screens::Taken`, its screen. The quick terminal is a throwaway,
    /// kept while temor runs and never saved; nor is a run tab.
    fn kept_panes(&self, screens: Screens) -> Vec<KeptPane> {
        self.slots
            .iter()
            .filter(|(pane, _)| !self.is_quick(**pane) && !self.run.panes.contains_key(pane))
            .filter_map(|(pane, slot)| {
                let slot = slot.borrow();
                let session = slot.session.as_ref()?;
                let mut checkpoint = session.checkpoint();
                self.keep_local(*pane, &mut checkpoint);
                let screen = match screens {
                    Screens::Taken => session.snapshot(),
                    Screens::Left => None,
                };
                Some(KeptPane {
                    pane: *pane,
                    checkpoint,
                    screen,
                })
            })
            .collect()
    }

    /// Writes the checkpoint of each pane whose state changed since it was last written, so a
    /// quit that never reaches the hub's drop still leaves the shells' state for the next launch.
    fn keep_checkpoints(&mut self) {
        let Some(dir) = self.state_dir.clone() else {
            return;
        };
        for kept in self.kept_panes(Screens::Left) {
            if self.on_disk.get(&kept.pane) != Some(&kept.checkpoint)
                && checkpoint::write(&dir, kept.pane, &kept.checkpoint).is_ok()
            {
                self.on_disk.insert(kept.pane, kept.checkpoint);
            }
        }
    }

    /// Writes each pane's checkpoint, with the daemon session it has.
    fn write_checkpoints(&self, dir: &std::path::Path) {
        for kept in self.kept_panes(Screens::Left) {
            let _ = checkpoint::write(dir, kept.pane, &kept.checkpoint);
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
        keep_at_quit(&dir, &self.kept_panes(Screens::Taken));
    }
}

/// Whether what is kept of a pane takes its screen too (only at quit: encoding it is not free).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screens {
    Taken,
    Left,
}

/// What is kept of one pane for the next launch.
struct KeptPane {
    pane: PaneId,
    checkpoint: temor_core::session::Checkpoint,
    screen: Option<temor_vt::Snapshot>,
}

/// Writes what a quit keeps, in place of what an earlier quit left: ids restart from the live
/// maximum, so an older file must not outlive this quit.
fn keep_at_quit(dir: &std::path::Path, panes: &[KeptPane]) {
    sidecar::clear(dir);
    checkpoint::clear(dir);
    crate::edge::replay::clear(dir);
    for kept in panes {
        let _ = checkpoint::write(dir, kept.pane, &kept.checkpoint);
        if let Some(screen) = &kept.screen {
            let _ = sidecar::write(dir, kept.pane, screen);
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
