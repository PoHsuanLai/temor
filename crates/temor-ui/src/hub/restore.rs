//! Starting the hub: every Space's last session when there is one, else a first tab in the Space
//! on screen. The Spaces file says which Spaces there are; each Space's session file says what
//! was open in it (a Space whose session file is missing falls back to the place the Spaces file
//! recalls for it).

use std::collections::BTreeSet;
use std::path::Path;

use ds::components::app::spaces::{SpaceId, Spaces};
use temor_core::notify::Focus;
use temor_core::space::{Place, Project, TemorSpaces};
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::restore::{Restored, VERSION, decode, launch_plan, viewer_plan};
use temor_core::workspace::{KeepClosed, Workspace};

use super::model::Hub;
use crate::edge::{SpacesStartup, Startup, load_hosts, load_session_for, load_spaces, sidecar};

impl Hub {
    /// Restores the saved Spaces (every tab's panes, in their folders, with their scrollback
    /// where a snapshot was kept) or opens a first tab.
    pub fn start(&mut self) {
        if let Some(dir) = self.state_dir.clone() {
            let hosts = load_hosts(&dir);
            self.ssh.book = hosts.book;
            self.remote_carry_over();
            self.ssh.restored = hosts.panes;
            self.book.spaces = self.read_spaces(&dir);
            self.book.active = self.book.spaces.current().id;
            let ids: Vec<SpaceId> = self.book.spaces.list().iter().map(|s| s.id).collect();
            let mut taken = BTreeSet::new();
            for id in ids {
                self.restore_space(&dir, id, &mut taken);
            }
            // A snapshot is good for one launch only: pane ids are reused after a restore.
            sidecar::clear(&dir);
            self.quiet_parked();
        }
        let restored = !self.slots.is_empty();
        // A first launch that was asked to run a program opens with its tab, not an empty shell.
        if let Some(run) = self.launch.first_run.take() {
            self.run_in_tab(run);
        }
        self.fill_empty();
        if !restored {
            self.first_hint = self.keyboard_pane();
        }
        self.refresh_probe();
        self.sweep_old_snapshots();
    }

    fn read_spaces(&mut self, dir: &Path) -> TemorSpaces {
        let first = || Spaces::first_run([Project::default()], Project::default);
        match load_spaces(dir) {
            Ok(SpacesStartup::Loaded(spaces)) => spaces,
            Ok(SpacesStartup::NoFile) => first(),
            Ok(SpacesStartup::Damaged) => {
                self.notices.push(
                    "Your Spaces file was damaged. It was set aside and a new one started."
                        .to_owned(),
                );
                first()
            }
            Err(err) => {
                self.notices.push(err.to_string());
                first()
            }
        }
    }

    /// Brings back the workspace of `id` and starts its panes. `taken` holds the panes already
    /// given to other Spaces: a file that reuses one of them is not restored.
    fn restore_space(&mut self, dir: &Path, id: SpaceId, taken: &mut BTreeSet<PaneId>) {
        let saved = match load_session_for(dir, id, KeepClosed::DEFAULT) {
            Ok(Startup::Restored(ws)) if !ws.tabs.place.tabs.is_empty() => Some(*ws),
            Ok(_) => from_place(&self.book.spaces.recall().of(id)),
            Err(err) => {
                self.notices.push(err.to_string());
                None
            }
        };
        let Some(ws) = saved else { return };
        let panes = ws.tabs.panes();
        if panes.iter().any(|pane| taken.contains(pane)) {
            self.notices.push(
                "A saved Space shared a terminal with another and was not restored.".to_owned(),
            );
            return;
        }
        taken.extend(panes);
        self.book.note(&ws);
        let plan = launch_plan(&ws);
        let viewers = viewer_plan(&ws);
        match id == self.book.active {
            true => self.ws = ws,
            false => {
                self.book.parked.insert(id, ws);
            }
        }
        self.with_space(id, |hub| {
            for spawn in plan {
                let snapshot = sidecar::take(dir, spawn.pane);
                hub.open_pane(&spawn, snapshot);
            }
            for open in viewers {
                hub.restore_viewer(open);
            }
        });
    }

    /// Only the Space on screen has the keyboard: the shells of the others are not "focused".
    fn quiet_parked(&mut self) {
        for ws in self.book.parked.values() {
            for pane in ws.tabs.panes() {
                if let Some(slot) = self.slots.get(&pane)
                    && let Some(session) = slot.borrow_mut().session.as_mut()
                {
                    session.set_focus(Focus::Unfocused);
                }
            }
        }
    }
}

/// A workspace from the place the Spaces file recalls: its tabs and folders, each shell starting
/// in the Space's folder. `None` when the place holds no tab.
fn from_place(place: &Place) -> Option<Workspace> {
    let text = serde_json::json!({ "version": VERSION, "place": place }).to_string();
    match decode(&text, KeepClosed::DEFAULT) {
        Restored::Loaded(ws) if !ws.tabs.place.tabs.is_empty() => Some(*ws),
        _ => None,
    }
}
