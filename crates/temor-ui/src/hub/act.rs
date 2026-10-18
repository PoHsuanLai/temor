//! Carrying out the workspace's answers: start a pane, take its scrollback, end it, forget what
//! left the recently closed list, and note that the session wants saving.

use std::cell::RefCell;
use std::rc::Rc;

use temor_core::notify::Focus;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::{Spawn, Start, WorkspaceIn, WorkspaceOut, workspace_step};
use temor_vt::Snapshot;

use super::model::{Hub, HubRef};
use super::say::refusal_text;
use crate::edge::sidecar;
use crate::session::Session;
use crate::view::launch::open_session_with;
use crate::view::slot::{Slot, repaint};

impl Hub {
    /// Runs one request through the workspace step and carries out what it answers.
    pub fn act(&mut self, input: WorkspaceIn) {
        if let Some(space) = self.parked_owner_of(&input) {
            return self.with_space(space, |hub| hub.act(input));
        }
        let input = self.in_project(input);
        self.book.lift(&mut self.ws);
        let input = self.ssh_prepare(input);
        let input = self.start_prepared(input);
        let input = self.measured(input);
        let at_home = self.split_starts_at_home(&input);
        let (ws, outs) = workspace_step(self.ws.clone(), input);
        self.ws = ws;
        self.book.note(&self.ws);
        for out in outs {
            match out {
                WorkspaceOut::Spawn(spawn) if at_home => {
                    let spawn = self.starting_at_home(spawn);
                    self.carry_out(WorkspaceOut::Spawn(spawn));
                }
                other => self.carry_out(other),
            }
        }
        self.agents_after_act();
        self.ssh.intent = None;
        // Closing the last tab leaves the window with a fresh one, as a browser does.
        if self.ws.tabs.place.tabs.is_empty() {
            self.act(WorkspaceIn::NewTab { cwd: None });
        }
    }

    pub(super) fn carry_out(&mut self, out: WorkspaceOut) {
        match out {
            WorkspaceOut::Spawn(spawn) => self.open_pane(&spawn, None),
            WorkspaceOut::Viewer(open) => self.viewer_opened(open),
            WorkspaceOut::Snapshot(pane) => self.keep_scrollback(pane),
            WorkspaceOut::End(pane) => self.end_pane(pane),
            WorkspaceOut::Forgotten(panes) => {
                for pane in panes {
                    self.recent.remove(&pane);
                    self.ssh.closed.remove(&pane);
                    if let Some(dir) = &self.state_dir {
                        sidecar::remove(dir, pane);
                    }
                }
            }
            WorkspaceOut::Focused(_) => self.refresh_probe(),
            WorkspaceOut::Tab(_) => {}
            WorkspaceOut::Refused(why) => self.notices.push(refusal_text(&why)),
            WorkspaceOut::Changed => self.pace.changed(),
            // An output a later core adds has nothing for the window to do yet.
            _ => {}
        }
    }

    /// Starts the program of `spawn` in a slot of its own. `restored` is a snapshot read from
    /// disk; otherwise a pane that is coming back from the recently closed list takes its own.
    /// A pane that cannot start still gets a slot (empty), and the reason is for the person.
    pub(super) fn open_pane(&mut self, spawn: &Spawn, restored: Option<Snapshot>) {
        if let Some(alias) = self.ssh_claims(spawn) {
            return self.begin_host(spawn, alias);
        }
        let snapshot = restored.or_else(|| match spawn.start {
            Start::Scrollback(pane) => self.recent.remove(&pane),
            Start::Fresh => None,
        });
        let start = self.space_start();
        let cwd = spawn.cwd.clone().or(start.cwd);
        let socket = self.open_socket(spawn.pane);
        // A program run in place of the shell brings its own environment, not the Space's.
        let run = self.run_claims(spawn.pane);
        let opened = open_session_with(
            &self.launch,
            self.palette,
            (spawn.pane, cwd.as_ref(), snapshot.as_ref()),
            self.events.clone(),
            run.as_ref().map_or(&start.env, |run| &run.env),
            socket.as_deref(),
            run.as_ref().map(|run| run.program()),
        );
        let mut slot = Slot {
            palette: self.palette,
            ..Slot::default()
        };
        match opened {
            Ok(mut session) => {
                // Only the pane that has the keyboard counts as focused (a long command that
                // finishes in another pane is worth a notice).
                session.set_focus(match self.keyboard_pane() == Some(spawn.pane) {
                    true => Focus::Focused,
                    false => Focus::Unfocused,
                });
                if let Some(command) = &spawn.relaunch {
                    type_command(&session, command.as_str());
                }
                slot.session = Some(session);
            }
            Err(err) => self.notices.push(err.to_string()),
        }
        self.slots.insert(spawn.pane, Rc::new(RefCell::new(slot)));
    }

    /// Takes the scrollback of a pane that is about to end, for a reopen.
    fn keep_scrollback(&mut self, pane: PaneId) {
        let snapshot = self
            .slots
            .get(&pane)
            .and_then(|slot| slot.borrow().session.as_ref().and_then(Session::snapshot));
        if let Some(snapshot) = snapshot {
            self.recent.insert(pane, snapshot);
        }
    }

    /// Ends a pane's program. Its scrollback stays only when the pane is in a closed tab.
    pub(super) fn end_pane(&mut self, pane: PaneId) {
        self.run_closed(pane);
        self.viewer_ended(pane);
        self.agent_pane_ended(pane);
        self.ssh_forget(pane);
        if let Some(slot) = self.slots.remove(&pane) {
            // The view may still hold the slot for a frame; the program ends now.
            slot.borrow_mut().session = None;
        }
        let in_closed_tab = self
            .ws
            .closed
            .tabs
            .iter()
            .any(|closed| closed.tab.tree.contains(pane));
        if !in_closed_tab {
            self.recent.remove(&pane);
        }
    }

    /// Starts a new shell in a pane whose program ended, in the folder it was last in.
    pub fn restart(&mut self, pane: PaneId) {
        if self.ssh_restart(pane) {
            return;
        }
        let start = self.space_start();
        let cwd = self.ws.cwds.get(&pane).cloned().or(start.cwd);
        let socket = self.open_socket(pane);
        // A tab that ran a program runs it again.
        self.run_restarted(pane);
        let run = self.run.panes.get(&pane).cloned();
        match open_session_with(
            &self.launch,
            self.palette,
            (pane, cwd.as_ref(), None),
            self.events.clone(),
            run.as_ref().map_or(&start.env, |run| &run.env),
            socket.as_deref(),
            run.as_ref().map(|run| run.program()),
        ) {
            Ok(session) => {
                if let Some(slot) = self.slots.get(&pane) {
                    slot.borrow_mut().session = Some(session);
                }
            }
            Err(err) => self.notices.push(err.to_string()),
        }
    }

    /// Only the pane that has the keyboard publishes its screen to the probe; it publishes at once.
    pub(super) fn refresh_probe(&mut self) {
        let keyboard = self.keyboard_pane();
        for (pane, slot) in &self.slots {
            slot.borrow_mut().probe = match Some(*pane) == keyboard {
                true => self.launch.probe.clone(),
                false => None,
            };
        }
        if let Some(slot) = keyboard.and_then(|pane| self.slots.get(&pane)) {
            repaint(slot);
        }
    }

    /// Makes `palette` the grid's colours in every pane; the first failure comes back as text.
    pub fn set_palette(&mut self, palette: temor_render::Palette) -> Option<String> {
        self.palette = palette;
        self.remote_palette();
        let mut failure = None;
        for slot in self.slots.values() {
            let said = slot.borrow_mut().set_palette(palette);
            failure = failure.or(said);
            repaint(slot);
        }
        failure
    }
}

/// Types a pinned tab's command into its new shell. The text is the person's own, but a control
/// character in it (a saved file edited by hand) is never sent.
fn type_command(session: &Session, command: &str) {
    if command.is_empty() || command.chars().any(char::is_control) {
        return;
    }
    session.pane().send(format!("{command}\n").as_bytes());
}

impl HubRef {
    /// Runs one request and reports the notices it left.
    pub fn act(&self, input: WorkspaceIn) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.act(input);
        std::mem::take(&mut hub.notices)
    }

    /// Restarts the shell of an ended pane.
    pub fn restart(&self, pane: PaneId) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.restart(pane);
        std::mem::take(&mut hub.notices)
    }

    /// Makes `palette` the grid's colours in every pane.
    pub fn set_palette(&self, palette: temor_render::Palette) -> Option<String> {
        self.0.borrow_mut().set_palette(palette)
    }
}
