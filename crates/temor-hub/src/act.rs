//! Carrying out the workspace's answers: start a pane, take its scrollback, end it, forget what
//! left the recently closed list, and note that the session wants saving.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use temor_client::model::pane::PaneFocus;
use temor_client::model::session::Checkpoint;
use temor_client::model::space::{Origin, spawn_plan};
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::vocab::text::{CommandText, Cwd};
use temor_client::model::workspace::{Spawn, Start, WorkspaceIn, WorkspaceOut};
use temor_client::ssh::HostAlias;
use temor_panes::vt::Snapshot;

use super::model::{Hub, HubRef};
use super::say::refusal_text;
use super::typing::{Submit, type_command};
use temor_pane_view::session::Session;
use temor_ui_base::launch::open_session_with;
use temor_ui_base::slot::{Slot, repaint};

/// What a request is asked with, beyond the request itself: decided before the workspace step
/// ([`Hub::prepare`]) and handed down to what carries out its answers, in place of fields set
/// on the hub around the call.
#[derive(Default)]
pub(super) struct ActCtx {
    /// The host the pane it starts is a session on.
    pub host: Option<HostAlias>,
    /// That host is shown offline: its pane waits for "Try anyway".
    pub offline: bool,
    /// Back or Forward walks the stage's pages: the page opened or closed is not a step.
    pub walking: bool,
    /// The screen the pane it starts shows (a reopened session's last); taken by the pane that
    /// shows it.
    pub screen: Option<Snapshot>,
    /// The pane it starts belongs in the home folder, not where it came from.
    pub at_home: bool,
}

impl ActCtx {
    /// A request that starts a session on `host`.
    pub(super) fn on_host(host: HostAlias, offline: bool) -> ActCtx {
        ActCtx {
            host: Some(host),
            offline,
            ..ActCtx::default()
        }
    }
}

impl Hub {
    /// Runs one request through the workspace step and carries out what it answers.
    pub fn act(&mut self, input: WorkspaceIn) {
        self.act_with(input, ActCtx::default());
    }

    /// [`Hub::act`] with what the request is asked with; answers what is left of it (the screen
    /// no pane took).
    pub(super) fn act_with(&mut self, input: WorkspaceIn, ctx: ActCtx) -> ActCtx {
        if let Some(space) = self.book.parked_owner_of(&input) {
            return self.with_space(space, |hub| hub.act_with(input, ctx));
        }
        let (input, mut ctx) = self.prepare(input, ctx);
        for out in self.book.step(input) {
            self.carry_out(out, &mut ctx);
        }
        self.agents_after_act();
        // Closing the last tab leaves the window with a fresh one, as a browser does.
        if self.book.ws().tabs.place.tabs.is_empty() {
            self.act(WorkspaceIn::NewTab { cwd: None });
        }
        ctx
    }

    /// The request as the workspace is to take it, and what it is asked with. The host comes
    /// first: a tab or split of a host pane is on that host, in its folder, so neither the
    /// Space's project folder nor the folder in front is put in for it.
    pub(super) fn prepare(&self, input: WorkspaceIn, ctx: ActCtx) -> (WorkspaceIn, ActCtx) {
        let (input, mut ctx) = self.ssh_prepare(input, ctx);
        let on_host = ctx.host.is_some();
        let input = match on_host {
            true => input,
            false => self.start_prepared(self.in_project(input)),
        };
        let input = self.measured(input);
        ctx.at_home = self.split_starts_at_home(&input);
        (input, ctx)
    }

    /// Runs `input` and answers the pane it opened, if it opened one: the pane that was not
    /// there before. A request the workspace refuses opens none.
    pub(super) fn act_for_new_pane(&mut self, input: WorkspaceIn) -> Option<PaneId> {
        self.act_for_new_pane_with(input, ActCtx::default()).0
    }

    /// [`Hub::act_for_new_pane`] with what the request is asked with, and what is left of it.
    pub(super) fn act_for_new_pane_with(
        &mut self,
        input: WorkspaceIn,
        ctx: ActCtx,
    ) -> (Option<PaneId>, ActCtx) {
        let before: BTreeSet<PaneId> = self.book.ws().tabs.panes().into_iter().collect();
        let left = self.act_with(input, ctx);
        let opened = self
            .book
            .ws()
            .tabs
            .panes()
            .into_iter()
            .find(|pane| !before.contains(pane));
        (opened, left)
    }

    /// Opens a tab in `cwd` (the default folder with `None`) and answers its pane.
    pub(super) fn open_tab(&mut self, cwd: Option<Cwd>) -> Option<PaneId> {
        self.act_for_new_pane(WorkspaceIn::NewTab { cwd })
    }

    pub(super) fn carry_out(&mut self, out: WorkspaceOut, ctx: &mut ActCtx) {
        match out {
            WorkspaceOut::Spawn(spawn) => {
                let spawn = match ctx.at_home {
                    true => self.starting_at_home(spawn),
                    false => spawn,
                };
                self.open_pane(&spawn, None, ctx);
            }
            WorkspaceOut::Viewer(open) => self.viewer_opened(open, ctx.walking),
            WorkspaceOut::Snapshot(pane) => self.keep_scrollback(pane),
            WorkspaceOut::End(pane) => {
                self.stage_closed(pane, ctx.walking);
                self.end_pane(pane);
            }
            WorkspaceOut::Forgotten(panes) => {
                for pane in panes {
                    self.forget_pane(pane);
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
    /// `ctx` says what the request that started it was asked with.
    pub(super) fn open_pane(
        &mut self,
        spawn: &Spawn,
        restored: Option<Snapshot>,
        ctx: &mut ActCtx,
    ) {
        if let Some(claim) = self.ssh_claims(spawn, ctx.host.take()) {
            return self.begin_host(spawn, claim, ctx.offline);
        }
        if self.local_wanted(spawn) {
            let resume = self
                .memory
                .get(spawn.pane)
                .and_then(|kept| kept.checkpoint.as_ref()?.daemon_session)
                .map(temor_client::remote::ids::RemoteSessionId);
            return self.begin_local(spawn, resume);
        }
        let restored = restored.or_else(|| ctx.screen.take());
        self.open_plain(spawn, restored);
    }

    /// Starts the program of `spawn` as a child of this window.
    pub(super) fn open_plain(&mut self, spawn: &Spawn, restored: Option<Snapshot>) {
        let snapshot = restored.or_else(|| match spawn.start {
            Start::Scrollback(pane) => self.memory.take(pane, |kept| kept.screen.take()),
            Start::Fresh => None,
        });
        let mut start = self.space_start();
        let kept = self.memory.take(spawn.pane, |kept| kept.checkpoint.take());
        let cwd = spawn
            .cwd
            .clone()
            .or_else(|| kept.as_ref().and_then(|kept| kept.cwd.clone().map(Cwd)))
            .or(start.cwd);
        let replay = kept
            .as_ref()
            .and_then(|kept| self.carry_env(&mut start.env, spawn.pane, kept));
        let socket = self.open_socket(spawn.pane);
        // A program run in place of the shell brings its own environment, not the Space's.
        let run = self.run_claims(spawn.pane);
        let opened = open_session_with(
            &self.launch,
            self.palette,
            (spawn.pane, cwd.as_ref(), snapshot.as_ref()),
            self.events.clone(),
            match &run {
                Some(run) => (&run.env, None),
                None => (&start.env, replay.as_deref()),
            },
            socket.as_deref(),
            run.as_ref().map(|run| run.program()),
        );
        let mut slot = Slot {
            palette: self.palette,
            ..Slot::default()
        };
        match opened {
            Ok(mut session) => {
                if snapshot.is_some() {
                    session.begin_restored(kept.map(|kept| kept.history).unwrap_or_default());
                }
                // Only the pane that has the keyboard counts as focused (a long command that
                // finishes in another pane is worth a notice).
                session.set_focus(self.focus_of(spawn.pane));
                if let Some(command) = &spawn.relaunch {
                    type_command(
                        &session,
                        &CommandText(command.as_str().to_owned()),
                        Submit::Enter,
                    );
                }
                self.hear(spawn.pane, &session);
                slot.session = Some(session);
            }
            Err(err) => self.notices.push(err.to_string()),
        }
        self.panes.record(spawn.pane).slot = Some(Rc::new(RefCell::new(slot)));
    }

    /// Gives a new shell what the person had set in the one it replaces: the variables that differ
    /// from what a shell starts with in this Space, set in `env`, and the aliases, in a script the
    /// new shell reads once at its first prompt (where it is returned).
    pub(super) fn carry_env(
        &self,
        env: &mut temor_panes::shell::ChildEnv,
        pane: PaneId,
        kept: &Checkpoint,
    ) -> Option<std::path::PathBuf> {
        let carried = kept.replay();
        let replay = self.state_dir.as_deref().and_then(|dir| {
            temor_client::keep::replay::write(
                dir,
                pane,
                &temor_panes::shell::alias_script(&carried.aliases),
            )
        });
        let base = self.launch.ambient.env.clone().overlay(env);
        let base: Vec<(String, String)> = base
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.to_string_lossy().into_owned(),
                )
            })
            .collect();
        for (key, value, origin) in spawn_plan(&base, &base, None, Some(&carried)).env {
            if origin == Origin::User {
                env.set(key, value);
            }
        }
        replay
    }

    /// Takes the scrollback of a pane that is about to end, for a reopen.
    fn keep_scrollback(&mut self, pane: PaneId) {
        let snapshot = self
            .panes
            .slot(pane)
            .and_then(|slot| slot.borrow().session.as_ref().and_then(Session::snapshot));
        if let Some(snapshot) = snapshot {
            // Kept under its session too, for the session's history page to reopen it from.
            if let (Some(dir), Some(id)) = (&self.state_dir, self.session_id(pane)) {
                let _ = temor_client::keep::screens::write(dir, id, snapshot.as_bytes());
            }
            self.memory.of(pane).screen = Some(snapshot);
        }
        let kept = self
            .panes
            .slot(pane)
            .and_then(|slot| slot.borrow().session.as_ref().map(Session::checkpoint));
        if let Some(kept) = kept {
            self.memory.of(pane).checkpoint = Some(kept);
        }
    }

    /// Ends a pane's program. Its scrollback stays only when the pane is in a closed tab.
    pub(super) fn end_pane(&mut self, pane: PaneId) {
        self.drop_pane(pane);
        let in_closed_tab = self
            .book
            .ws()
            .closed
            .tabs
            .iter()
            .any(|closed| closed.tab.tree.contains(pane));
        if !in_closed_tab {
            self.memory.take(pane, |kept| kept.screen.take());
        }
    }

    /// Everything a pane that is gone leaves running: its run tab's launch stops waiting, its
    /// viewer, agent sockets and host state go, and so does its session. The one path for a pane
    /// that closes and for a pane whose Space is deleted. Its scrollback is the caller's.
    pub(super) fn drop_pane(&mut self, pane: PaneId) {
        let found = self.panes.get_mut(pane).and_then(|r| r.run.take());
        self.run_closed(found);
        self.viewer_ended(pane);
        self.agent_pane_ended(pane);
        self.ssh_forget(pane);
        if let Some(slot) = self.panes.remove(pane).and_then(|record| record.slot) {
            // The view may still hold the slot for a frame; the program ends now.
            slot.borrow_mut().session = None;
        }
    }

    /// What is kept of a pane that can no longer come back: its scrollback, its place in the
    /// closed list of hosts, and its sidecar file.
    pub(super) fn forget_pane(&mut self, pane: PaneId) {
        self.memory.forget(pane);
        self.files.forget(self.state_dir.as_deref(), pane);
    }

    /// Tells `pane`'s session whether it has the keyboard, if it has a session.
    pub(super) fn set_pane_focus(&self, pane: PaneId, focus: PaneFocus) {
        let _ = self.live_mut(pane, |session| session.set_focus(focus));
    }

    /// Whether `pane`'s session counts as focused: only the pane with the keyboard does.
    pub(super) fn focus_of(&self, pane: PaneId) -> PaneFocus {
        match self.keyboard_pane() == Some(pane) {
            true => PaneFocus::Focused,
            false => PaneFocus::Unfocused,
        }
    }

    /// Starts a new shell in a pane whose program ended, in the folder it was last in.
    pub fn restart(&mut self, pane: PaneId) {
        if self.ssh_restart(pane) {
            return;
        }
        let start = self.space_start();
        let cwd = self.book.ws().cwds.get(&pane).cloned().or(start.cwd);
        let socket = self.open_socket(pane);
        // A tab that ran a program runs it again.
        self.run_restarted(pane);
        let run = self.run_of(pane).cloned();
        match open_session_with(
            &self.launch,
            self.palette,
            (pane, cwd.as_ref(), None),
            self.events.clone(),
            (run.as_ref().map_or(&start.env, |run| &run.env), None),
            socket.as_deref(),
            run.as_ref().map(|run| run.program()),
        ) {
            Ok(mut session) => {
                session.set_focus(self.focus_of(pane));
                self.hear(pane, &session);
                if let Some(slot) = self.panes.slot(pane) {
                    slot.borrow_mut().session = Some(session);
                }
            }
            Err(err) => self.notices.push(err.to_string()),
        }
    }

    /// Only the pane that has the keyboard publishes its screen to the probe; it publishes at once.
    pub(super) fn refresh_probe(&mut self) {
        let keyboard = self.keyboard_pane();
        for (pane, slot) in self.panes.slots() {
            slot.borrow_mut().probe = match Some(pane) == keyboard {
                true => self.launch.probe.clone(),
                false => None,
            };
        }
        if let Some(slot) = keyboard.and_then(|pane| self.panes.slot(pane)) {
            repaint(slot);
        }
    }

    /// Makes `palette` the grid's colours in every pane; the first failure comes back as text.
    pub fn set_palette(&mut self, palette: temor_panes::render::Palette) -> Option<String> {
        self.palette = palette;
        self.remote_palette();
        let mut failure = None;
        for slot in self.panes.slots().map(|(_, slot)| slot) {
            let said = slot.borrow_mut().set_palette(palette);
            failure = failure.or(said);
            repaint(slot);
        }
        failure
    }
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
    pub fn set_palette(&self, palette: temor_panes::render::Palette) -> Option<String> {
        self.0.borrow_mut().set_palette(palette)
    }
}
