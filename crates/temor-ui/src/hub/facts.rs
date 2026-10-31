//! The pane's context facts in the hub (PLAN P16): when to read them again (`facts_step`), and
//! what the sidebar row and the chips show. The client reads them on a thread of its own
//! (`temor_client::facts`).
//! The window's thread only steps the rule and stores results; it never waits for git.

use std::collections::BTreeMap;
use std::sync::Arc;

use temor_client::model::facts::{Facts, FactsIn, FactsOut, FactsState, facts_step};
use temor_client::model::vocab::ids::PaneId;
use temor_client::remote::ids::RemoteSessionId;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::model::Hub;
use temor_client::facts::{FactsEnv, ask_host_later, read_later};
use temor_client::remote::conn::Conn;

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

/// The window's facts: what the panes last read, and the way back to the window.
pub struct FactsHub {
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
    /// Facts for a window. The virtualenv comes from a `.venv` folder only: one activated
    /// inside a shell is not visible to the window until the shell reports it (FOLLOWUPS).
    pub fn new() -> FactsHub {
        let virtual_env = None;
        let (done, inbox) = unbounded_channel();
        FactsHub {
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

    /// Reads `dir` off the window's thread and sends the result to the window. False when the
    /// read could not start.
    fn spawn(&self, pane: PaneId, dir: String) -> bool {
        let done = self.done.clone();
        // A closed channel means the window is gone.
        read_later(dir, self.env.clone(), move |facts| {
            let _ = done.send(Ready {
                pane,
                facts: Some(facts),
            });
        })
    }

    /// Asks the helper of a host for the facts of `session`'s folder. False when the ask could
    /// not start.
    fn spawn_host(&self, pane: PaneId, source: (Arc<Conn>, RemoteSessionId)) -> bool {
        let done = self.done.clone();
        ask_host_later(source, move |facts| {
            let _ = done.send(Ready { pane, facts });
        })
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

    /// The helper connection and session of a pane on a host's helper (Tier B), once it is
    /// attached. A host without the helper (Tier A) has no chips: nothing there can read its
    /// folder but the shell.
    fn helper_facts_source(&self, pane: PaneId) -> Option<(Arc<Conn>, RemoteSessionId)> {
        let remote = self.remote.panes.get(&pane)?;
        let session = remote.session?;
        Some((self.remote.helper_conn(&remote.alias)?, session))
    }

    /// Starts a read of the pane's folder: on this computer, or through the host's helper for a
    /// pane on it. A pane on a host without a helper, and a pane with no folder yet, have
    /// nothing to read: both count the run as done at once.
    fn facts_run(&mut self, pane: PaneId) {
        if self.host_of(pane).is_some() {
            let started = self
                .helper_facts_source(pane)
                .is_some_and(|source| self.facts.spawn_host(pane, source));
            if !started {
                self.facts_in(pane, FactsIn::RunDone);
            }
            return;
        }
        match self.pane_folder(pane) {
            Some(dir) => {
                self.plugin_facts_start(pane, &dir);
                // A read that could not start is done: the facts stay as they are.
                if !self.facts.spawn(pane, dir) {
                    self.facts_in(pane, FactsIn::RunDone);
                }
            }
            None => {
                // The rule only ever answers `Run` to `RunDone` after a trigger, so this cannot
                // loop: a `Stale` run is started once more, finds no folder, and goes idle.
                self.facts_in(pane, FactsIn::RunDone);
            }
        }
    }

    /// A finished read: stored, and the rule told. Results for a pane that is gone are dropped.
    /// True when it changed what the window shows.
    pub(super) fn facts_ready(&mut self, ready: Ready) -> bool {
        let Ready { pane, facts } = ready;
        if !self.slots.contains_key(&pane) {
            self.facts.panes.remove(&pane);
            return false;
        }
        let entry = self.facts.panes.entry(pane).or_default();
        let changed = match facts {
            Some(facts) => {
                let changed = entry.facts != facts;
                entry.facts = facts;
                changed
            }
            None => false,
        };
        self.facts_in(pane, FactsIn::RunDone);
        changed
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
    pub(super) fn pane_folder(&self, pane: PaneId) -> Option<String> {
        self.live(pane, |s| s.cwd())
            .flatten()
            .or_else(|| self.book.ws.cwds.get(&pane).map(|c| c.as_str().to_owned()))
    }
}
