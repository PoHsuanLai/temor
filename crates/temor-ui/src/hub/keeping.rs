//! The window's side of what is kept between launches (PLAN §4b, §7). The keeping is the
//! client's ([`temor_client::keep`]); the window hands it the panes' state, starts the panes a
//! restore says to, and asks on a timer whether a change is due to be written (never in the
//! render path). The last write and the panes' screens happen when the hub is dropped, at quit.

use std::collections::BTreeSet;
use std::time::Instant;

use temor_client::keep::{KeptPane, Laying, Pace, PaneFiles, Starting, Through};
use temor_client::model::notify::Focus;
use temor_client::model::vocab::ids::PaneId;

use super::model::Hub;

impl Hub {
    /// Restores the kept Spaces (every tab's panes, in their folders, with their scrollback
    /// where a screen was kept) or opens a first tab.
    pub fn start(&mut self) {
        let dir = self.state_dir.clone();
        let client = self.client().cloned();
        if let Some(client) = &client {
            if let Some(dir) = &dir {
                self.kept.import(client, dir);
            }
            let sessions = self.kept.sessions(client);
            self.carry_sessions(sessions);
            let hosts = self.kept.hosts(client);
            self.ssh.book = hosts.book;
            self.remote_carry_over();
            self.ssh.restored = hosts.panes;
        }
        for starting in self.kept.restore(client.as_ref(), &mut self.book) {
            self.start_space(dir.as_deref(), starting);
        }
        let notices = self.kept.take_notices();
        self.notices.extend(notices);
        if let Some(dir) = &dir {
            // A screen is good for one launch only: pane ids are reused after a restore.
            PaneFiles::clear(dir);
        }
        self.quiet_parked();
        // A first launch that was asked to run a program opens with its tab, not an empty shell.
        if let Some(run) = self.launch.first_run.take() {
            self.run_in_tab(run);
        }
        self.fill_empty();
        self.refresh_probe();
    }

    /// Starts the panes and viewers of a Space brought back, each pane with the screen and the
    /// shell state kept for it.
    fn start_space(&mut self, dir: Option<&std::path::Path>, starting: Starting) {
        self.with_space(starting.space, |hub| {
            for spawn in starting.panes {
                let (screen, kept) = match dir {
                    Some(dir) => PaneFiles::take(dir, spawn.pane),
                    None => (None, None),
                };
                if let Some(kept) = kept {
                    hub.carried.insert(spawn.pane, kept);
                }
                hub.open_pane(&spawn, screen.map(temor_panes::vt::Snapshot::from_bytes));
            }
            for open in starting.viewers {
                hub.restore_viewer(open);
            }
        });
    }

    /// Only the Space on screen has the keyboard: the shells of the others are not "focused".
    fn quiet_parked(&mut self) {
        let parked: Vec<PaneId> = self
            .book
            .parked()
            .flat_map(|(_, ws)| ws.tabs.panes())
            .collect();
        for pane in parked {
            self.set_pane_focus(pane, Focus::Unfocused);
        }
    }

    /// Writes what is kept if a change is waiting and the pace allows it. Returns how long to
    /// wait before asking again, when a write is waiting for its turn.
    pub fn poll_save(&mut self, now: Instant) -> Option<std::time::Duration> {
        if let Some(dir) = self.state_dir.clone() {
            let panes = self.kept_panes(false);
            self.files.keep_changed(&dir, panes);
        }
        match self.pace.poll(now) {
            Pace::Idle => None,
            Pace::WaitFor(wait) => Some(wait),
            Pace::WriteNow => {
                self.lay_all(Through::Client);
                // Quitting from the system's menu ends the process without dropping the hub,
                // so what a next launch needs from each pane is kept now.
                if let Some(dir) = &self.state_dir {
                    PaneFiles::keep_all(dir, &self.kept_panes(false));
                }
                self.pace.wrote(now);
                None
            }
        }
    }

    /// Hands the client the layout of every Space, the hosts and the panes' sessions.
    fn lay_all(&mut self, through: Through) {
        let Some(client) = self.client().cloned() else {
            return;
        };
        // A tab that ran a program (`temor -e`) is not brought back: it was that launch's.
        let leave: BTreeSet<PaneId> = self.run.panes.keys().copied().collect();
        let hosts = self.hosts_file();
        let sessions = self.sessions_to_carry();
        let laying = Laying {
            book: &self.book,
            leave: &leave,
            hosts: &hosts,
            sessions: &sessions,
        };
        self.kept.lay_all(&client, laying, through);
    }

    /// What is kept of each pane that outlives the window: its checkpoint (with the daemon
    /// session it has) and, with `screens`, its screen. The quick terminal is a throwaway, kept
    /// while temor runs and never saved; nor is a run tab.
    fn kept_panes(&self, screens: bool) -> Vec<KeptPane> {
        self.slots
            .iter()
            .filter(|(pane, _)| !self.is_quick(**pane) && !self.run.panes.contains_key(pane))
            .filter_map(|(pane, slot)| {
                let slot = slot.borrow();
                let session = slot.session.as_ref()?;
                let mut checkpoint = session.checkpoint();
                self.keep_local(*pane, &mut checkpoint);
                Some(KeptPane {
                    pane: *pane,
                    checkpoint,
                    screen: screens
                        .then(|| {
                            session
                                .snapshot()
                                .map(temor_panes::vt::Snapshot::into_bytes)
                        })
                        .flatten(),
                })
            })
            .collect()
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        self.lay_all(Through::Now);
        let Some(dir) = self.state_dir.clone() else {
            return;
        };
        // The links go, the host sessions stay: quitting detaches (D18).
        self.remote_quit();
        PaneFiles::keep_at_quit(&dir, &self.kept_panes(true));
    }
}

/// How often the window asks whether a change is waiting to be written.
const SAVE_TICK: std::time::Duration = std::time::Duration::from_millis(500);

/// Starts the timer that writes what is kept for as long as the window lives.
pub fn spawn_save_loop(hub: super::model::HubRef) {
    dioxus::prelude::spawn(async move {
        loop {
            ds::base::time::clock::sleep(SAVE_TICK).await;
            let _ = hub.0.borrow_mut().poll_save(Instant::now());
        }
    });
}
