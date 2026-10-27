//! Starting the hub: every Space's last session when there is one, else a first tab in the Space
//! on screen. The kept Spaces say which Spaces there are; each Space's kept rows say what was
//! open in it (a Space with none falls back to the place the Spaces recall for it). They are kept
//! in the store (`layout`); the files an older temor wrote are read into it first, once.

use std::collections::BTreeSet;
use std::path::Path;

use ds::components::app::spaces::SpaceId;
use temor_core::notify::Focus;
use temor_core::space::Place;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::restore::{Restored, VERSION, decode, launch_plan, viewer_plan};
use temor_core::workspace::{KeepClosed, Workspace};

use super::model::Hub;
use crate::edge::{checkpoint, sidecar};

impl Hub {
    /// Restores the saved Spaces (every tab's panes, in their folders, with their scrollback
    /// where a snapshot was kept) or opens a first tab.
    pub fn start(&mut self) {
        let dir = self.state_dir.clone();
        if let Some(dir) = &dir {
            self.import_files(dir);
        }
        let sessions = self.laid_sessions();
        self.carry_sessions(sessions);
        let hosts = self.laid_hosts();
        self.ssh.book = hosts.book;
        self.remote_carry_over();
        self.ssh.restored = hosts.panes;
        if let Some(spaces) = self.laid_spaces() {
            self.book.spaces = spaces;
        }
        self.book.active = self.book.spaces.current().id;
        let ids: Vec<SpaceId> = self.book.spaces.list().iter().map(|s| s.id).collect();
        let mut taken = BTreeSet::new();
        for id in ids {
            self.restore_space(dir.as_deref(), id, &mut taken);
        }
        if let Some(dir) = &dir {
            // A snapshot is good for one launch only: pane ids are reused after a restore.
            sidecar::clear(dir);
            checkpoint::clear(dir);
        }
        self.quiet_parked();
        // A first launch that was asked to run a program opens with its tab, not an empty shell.
        if let Some(run) = self.launch.first_run.take() {
            self.run_in_tab(run);
        }
        self.fill_empty();
        self.refresh_probe();
    }

    /// Brings back the workspace of `id` and starts its panes. `taken` holds the panes already
    /// given to other Spaces: a file that reuses one of them is not restored.
    fn restore_space(&mut self, dir: Option<&Path>, id: SpaceId, taken: &mut BTreeSet<PaneId>) {
        let saved = match self.laid_space(id) {
            Some(ws) if !ws.tabs.place.tabs.is_empty() => Some(ws),
            _ => from_place(&self.book.spaces.recall().of(id)),
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
                let snapshot = dir.and_then(|dir| sidecar::take(dir, spawn.pane));
                if let Some(kept) = dir.and_then(|dir| checkpoint::take(dir, spawn.pane)) {
                    hub.carried.insert(spawn.pane, kept);
                }
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
                self.set_pane_focus(pane, Focus::Unfocused);
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
