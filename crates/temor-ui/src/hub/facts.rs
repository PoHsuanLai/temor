//! The pane's context facts in the hub (PLAN P16): when to read them again (`facts_step`), the
//! worker thread that reads them (`edge::facts`), and what the sidebar row and the chips show.
//! The window's thread only steps the rule and stores results; it never waits for git.

use std::collections::BTreeMap;
use std::sync::Arc;

use temor_core::facts::{Facts, FactsIn, FactsOut, FactsState, facts_step};
use temor_core::vocab::ids::PaneId;
use temor_core::worktree::Git;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::model::Hub;
use crate::edge::facts::{FactsEnv, read_facts};
use crate::edge::git::ProcessGit;
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
    facts: Facts,
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
    /// without it the branch and changes are left out. `VIRTUAL_ENV` is read from the
    /// environment the shells start with.
    pub fn new(launch: &Launch) -> FactsHub {
        let path = launch
            .ambient
            .env
            .get("PATH")
            .map_or_else(|| "/usr/bin:/bin".into(), std::ffi::OsStr::to_os_string);
        let git = temor_pty::find_program("git", &path)
            .map(|binary| Arc::new(ProcessGit::new(binary)) as Arc<dyn Git>);
        let virtual_env = launch
            .ambient
            .env
            .get("VIRTUAL_ENV")
            .map(|v| v.to_string_lossy().into_owned());
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
                let facts = read_facts(git.as_deref(), &dir, &env);
                // A closed channel means the window is gone.
                let _ = done.send(Ready { pane, facts });
            });
    }
}

impl Hub {
    /// Tells the rule what happened in `pane` and starts a read if it says to.
    pub(super) fn facts_in(&mut self, pane: PaneId, input: FactsIn) {
        let entry = self.facts.panes.entry(pane).or_default();
        let (state, out) = facts_step(entry.state, input);
        entry.state = state;
        if out == FactsOut::Run {
            self.facts_run(pane);
        }
    }

    /// Starts a read of the pane's folder. A pane on a host has no local folder to read; a pane
    /// with no folder yet has nothing to read: both count the run as done at once.
    fn facts_run(&mut self, pane: PaneId) {
        let dir = match self.host_of(pane) {
            Some(_) => None,
            None => self.pane_folder(pane),
        };
        match dir {
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
        let shown = match entry.facts == facts {
            true => Shown::Same,
            false => Shown::Changed,
        };
        entry.facts = facts;
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
