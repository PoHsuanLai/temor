//! The quick terminal in the window (PLAN §7): one chord shows a small throwaway terminal over
//! the window and the same chord hides it again, its shell still running. It is not saved with
//! the session; "Move to Space" promotes it into a real tab without restarting the shell. A
//! global hotkey (one that works while another app has the keys) is the desktop's, not the
//! window's.

use ds::style::space::list::SpaceId;
use temor_core::quick::{QuickIn, QuickOut, QuickState, adopt, quick_step};
use temor_core::vocab::ids::{PaneId, TabId};
use temor_core::vocab::text::Cwd;
use temor_core::workspace::{Spawn, Start};

use super::desk::Desk;
use super::model::{Hub, HubRef};

impl Hub {
    /// The quick terminal's pane while it is shown over the window.
    pub fn quick_shown(&self) -> Option<PaneId> {
        match &self.quick {
            QuickState::Open(term) => Some(term.pane),
            QuickState::Closed | QuickState::Hidden(_) => None,
        }
    }

    /// Whether `pane` is the quick terminal's, shown or hidden. Its keys and its events are not
    /// the workspace's.
    pub fn is_quick(&self, pane: PaneId) -> bool {
        match &self.quick {
            QuickState::Open(term) | QuickState::Hidden(term) => term.pane == pane,
            QuickState::Closed => false,
        }
    }

    /// The chord: show it (starting a shell in the folder of the pane in front the first time),
    /// or hide it and keep the shell.
    pub fn quick_toggle(&mut self) {
        let pane = match self.quick {
            QuickState::Closed => match self.mint_pane() {
                Some(pane) => pane,
                None => {
                    self.notices
                        .push("The quick terminal could not be opened.".to_owned());
                    return;
                }
            },
            // Not used: an open or hidden terminal keeps the pane it has.
            QuickState::Open(_) | QuickState::Hidden(_) => PaneId(0),
        };
        let cwd = self
            .keyboard_pane()
            .and_then(|front| self.ws.cwds.get(&front).cloned());
        self.quick_input(QuickIn::Toggle { pane, cwd });
    }

    /// "Move to Space": keep the running terminal as a tab.
    pub fn quick_move(&mut self, space: SpaceId) {
        self.quick_input(QuickIn::MoveToSpace(space));
    }

    /// Gives the quick terminal a report or a request and carries out what it asks.
    pub(super) fn quick_input(&mut self, input: QuickIn) {
        let (state, outs) = quick_step(std::mem::take(&mut self.quick), input);
        self.quick = state;
        let asked = !outs.is_empty();
        for out in outs {
            match out {
                QuickOut::Spawn { pane, cwd } => self.open_pane(&quick_spawn(pane, cwd), None),
                QuickOut::End(pane) => self.end_pane(pane),
                QuickOut::Promote { pane, space, cwd } => self.adopt_quick_pane(space, pane, cwd),
                // The overlay follows `quick_shown`; the shell is not touched.
                QuickOut::Show(_) | QuickOut::Hidden => {}
            }
        }
        if asked {
            self.refresh_probe();
        }
    }

    /// The next pane id, from the counter every pane takes its id from.
    fn mint_pane(&mut self) -> Option<PaneId> {
        let next = self.ws.next_pane.checked_add(1)?;
        let pane = PaneId(self.ws.next_pane);
        self.ws.next_pane = next;
        Some(pane)
    }

    /// Gives the running quick pane to the window as a tab of its own.
    // TODO(orchestrator): with per-Space workspaces this is the spaces lane's
    // `Hub::adopt_into_space(space, pane, cwd)`; until then the one workspace takes it.
    fn adopt_quick_pane(&mut self, _space: SpaceId, pane: PaneId, cwd: Option<Cwd>) {
        match adopt(&self.ws, pane, cwd) {
            Ok((ws, outs)) => {
                self.ws = ws;
                for out in outs {
                    self.carry_out(out);
                }
            }
            Err(_) => {
                self.end_pane(pane);
                self.notices
                    .push("The quick terminal could not be kept as a tab.".to_owned());
            }
        }
    }
}

/// The start of the quick terminal's shell: a fresh one, in no tab.
fn quick_spawn(pane: PaneId, cwd: Option<Cwd>) -> Spawn {
    Spawn {
        pane,
        tab: TabId(0),
        cwd,
        relaunch: None,
        start: Start::Fresh,
    }
}

impl HubRef {
    /// The quick terminal's pane while it is shown.
    pub fn quick_shown(&self) -> Option<PaneId> {
        self.0.borrow().quick_shown()
    }

    /// Whether `pane` is the quick terminal's.
    pub fn is_quick(&self, pane: PaneId) -> bool {
        self.0.borrow().is_quick(pane)
    }
}

impl Desk {
    /// The quick terminal's chord.
    pub fn quick_toggle(&self) {
        self.settle(Hub::quick_toggle);
    }

    /// "Move to Space".
    pub fn quick_move(&self, space: SpaceId) {
        self.settle(|hub| hub.quick_move(space));
    }
}
