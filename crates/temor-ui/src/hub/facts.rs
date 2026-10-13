//! The pane's context facts in the hub (PLAN P16): when to read them again (`facts_step`), the
//! worker thread that reads them (`edge::facts`), and what the sidebar row and the chips show.
//! The window's thread only steps the rule and stores results; it never waits for git.

use std::collections::BTreeMap;
use std::sync::Arc;

use temor_core::facts::{Facts, FactsIn, FactsOut, FactsState, facts_step};
use temor_core::vocab::ids::PaneId;
use temor_core::worktree::Git;
use temor_wire::ids::SessionId;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::model::Hub;
use crate::edge::facts::{FactsEnv, read_facts};
use crate::edge::facts_host::facts_from_host;
use crate::edge::git::ProcessGit;
use crate::edge::remote_conn::Conn;
use crate::view::launch::Launch;

/// Whether a result changed what the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shown {
    Same,
    Changed,
}

/// A finished read.
#[derive(Debug)]
pub struct Ready {
    pane: PaneId,
    /// What was read; none when the read failed, and what the pane showed stays.
    facts: Option<Facts>,
}

/// One pane's rule state and what was last read.
#[derive(Debug, Default)]
struct PaneFacts {
    state: FactsState,
    facts: Facts,
}

/// The window's facts: git (if found), what the panes last read, and the way back to the window.
pub struct FactsHub {
    git: Option<Arc<dyn Git>>,
    env: FactsEnv,
    panes: BTreeMap<PaneId, PaneFacts>,
    done: UnboundedSender<Ready>,
    inbox: Option<UnboundedReceiver<Ready>>,
}

impl std::fmt::Debug for FactsHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FactsHub")
            .field("panes", &self.panes.len())
            .finish_non_exhaustive()
    }
}

impl FactsHub {
    /// Facts for a window started from `launch`. git is looked for on the environment's `PATH`;
    /// without it the branch and changes are left out. The virtualenv comes from a `.venv`
    /// folder only: one activated inside a shell is not visible to the window until the shell
    /// reports it (FOLLOWUPS).
    pub fn new(launch: &Launch) -> FactsHub {
        let path = launch
            .ambient
            .env
            .get("PATH")
            .map_or_else(|| "/usr/bin:/bin".into(), std::ffi::OsStr::to_os_string);
        let git = temor_pty::find_program("git", &path)
            .map(|binary| Arc::new(ProcessGit::new(binary)) as Arc<dyn Git>);
        let virtual_env = None;
        let (done, inbox) = unbounded_channel();
        FactsHub {
            git,
            env: FactsEnv { virtual_env },
            panes: BTreeMap::new(),
            done,
            inbox: Some(inbox),
        }
    }

    /// The window's loop takes the receiving end once.
    pub fn take_inbox(&mut self) -> Option<UnboundedReceiver<Ready>> {
        self.inbox.take()
    }

    /// Reads `dir` on a thread of its own and sends the result to the window.
    fn spawn(&self, pane: PaneId, dir: String) {
        let (git, env, done) = (self.git.clone(), self.env.clone(), self.done.clone());
        // If the thread cannot start the run never reports; `facts_step` was told it is in
        // flight, so this pane's facts stay as they are until the window restarts the pane.
        let _ = std::thread::Builder::new()
            .name("temor-facts".to_owned())
            .spawn(move || {
                let facts = Some(read_facts(git.as_deref(), &dir, &env));
                // A closed channel means the window is gone.
                let _ = done.send(Ready { pane, facts });
            });
    }

    /// Asks the helper of a host for the facts of `session`'s folder, on a thread of its own.
    fn spawn_host(&self, pane: PaneId, (conn, session): (Arc<Conn>, SessionId)) {
        let done = self.done.clone();
        // As for a local read: if the thread cannot start, the pane's facts stay as they are.
        let _ = std::thread::Builder::new()
            .name("temor-facts-host".to_owned())
            .spawn(move || {
                let facts = conn
                    .client()
                    .folder_facts(session)
                    .ok()
                    .map(|sent| facts_from_host(&sent));
                let _ = done.send(Ready { pane, facts });
            });
    }
}

impl Hub {
    /// Tells the rule what happened in `pane` and starts a read if it says to.
    pub(super) fn facts_in(&mut self, pane: PaneId, input: FactsIn) {
        self.mark_host_site(pane);
        let entry = self.facts.panes.entry(pane).or_default();
        let (state, out) = facts_step(entry.state, input);
        entry.state = state;
        if out == FactsOut::Run {
            self.facts_run(pane);
        }
    }

    /// A pane that is a session on a host files for another computer: the paths it prints mean
    /// nothing here. The pane's first prompt or folder report comes before any click on it.
    fn mark_host_site(&self, pane: PaneId) {
        if self.host_of(pane).is_none() {
            return;
        }
        if let Some(slot) = self.slots.get(&pane)
            && let Some(session) = slot.borrow_mut().session.as_mut()
        {
            session.set_site(temor_core::shelf::PaneSite::Remote);
        }
    }

    /// The helper connection and session of a pane on a host's helper (Tier B), once it is
    /// attached. A host without the helper (Tier A) has no chips: nothing there can read its
    /// folder but the shell.
    fn helper_facts_source(&self, pane: PaneId) -> Option<(Arc<Conn>, SessionId)> {
        let remote = self.remote.panes.get(&pane)?;
        let session = remote.session?;
        Some((self.remote.helper_conn(&remote.alias)?, session))
    }

    /// Starts a read of the pane's folder: on this computer, or through the host's helper for a
    /// pane on it. A pane on a host without a helper, and a pane with no folder yet, have
    /// nothing to read: both count the run as done at once.
    fn facts_run(&mut self, pane: PaneId) {
        if self.host_of(pane).is_some() {
            match self.helper_facts_source(pane) {
                Some(source) => self.facts.spawn_host(pane, source),
                None => self.facts_in(pane, FactsIn::RunDone),
            }
            return;
        }
        match self.pane_folder(pane) {
            Some(dir) => self.facts.spawn(pane, dir),
            None => {
                // The rule only ever answers `Run` to `RunDone` after a trigger, so this cannot
                // loop: a `Stale` run is started once more, finds no folder, and goes idle.
                self.facts_in(pane, FactsIn::RunDone);
            }
        }
    }

    /// A finished read: stored, and the rule told. Results for a pane that is gone are dropped.
    pub(super) fn facts_ready(&mut self, ready: Ready) -> Shown {
        let Ready { pane, facts } = ready;
        if !self.slots.contains_key(&pane) {
            self.facts.panes.remove(&pane);
            return Shown::Same;
        }
        let entry = self.facts.panes.entry(pane).or_default();
        let shown = match facts {
            Some(facts) if entry.facts != facts => {
                entry.facts = facts;
                Shown::Changed
            }
            Some(_) | None => Shown::Same,
        };
        self.facts_in(pane, FactsIn::RunDone);
        shown
    }

    /// The facts last read for the pane (empty before the first read).
    pub fn facts_of(&self, pane: PaneId) -> Facts {
        self.facts
            .panes
            .get(&pane)
            .map(|p| p.facts.clone())
            .unwrap_or_default()
    }

    /// The pane's folder as its chip says it (the home folder as `~`). None on another
    /// computer: its paths are not under this home, and its folders are not read here.
    fn chip_folder(&self, pane: PaneId) -> Option<String> {
        if self.host_of(pane).is_some() {
            return None;
        }
        let home = &self.launch.ambient.home;
        self.pane_folder(pane)
            .map(|path| super::rows::tidy(&path, home))
    }

    /// The pane's folder: the shell's report, else where the pane started.
    fn pane_folder(&self, pane: PaneId) -> Option<String> {
        self.live(pane, |s| s.cwd().map(str::to_owned))
            .flatten()
            .or_else(|| self.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()))
    }
}

impl super::HubRef {
    /// The facts last read for the pane.
    pub fn facts_of(&self, pane: PaneId) -> Facts {
        self.0.borrow().facts_of(pane)
    }

    /// The pane's folder as its chip says it, when it is on this computer.
    pub fn chip_folder(&self, pane: PaneId) -> Option<String> {
        self.0.borrow().chip_folder(pane)
    }
}

/// Carries finished reads into the hub for as long as the window lives, and redraws when one
/// changed what is shown. Call once, next to the pane event loop.
pub fn spawn_facts_loop(desk: super::Desk) {
    use dioxus::prelude::spawn;
    let Some(mut inbox) = desk.hub.0.borrow_mut().facts.take_inbox() else {
        return;
    };
    spawn(async move {
        while let Some(ready) = inbox.recv().await {
            let shown = desk.hub.0.borrow_mut().facts_ready(ready);
            if shown == Shown::Changed {
                desk.bump();
            }
        }
    });
}
