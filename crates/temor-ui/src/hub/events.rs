//! Hearing the panes. Every pane's events arrive on one channel tagged with its id; the window
//! drains it, gives each event to its pane's session, and reacts to what the sessions report
//! (repaint, redraw the chrome, note a finished command or a new folder in the workspace).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use temor_client::model::facts::FactsIn;
use temor_client::model::quick::QuickIn;
use temor_client::model::vocab::ids::PaneId;
use temor_client::model::vocab::text::{Cwd, TabTitle};
use temor_client::model::workspace::{TabsIn, WorkspaceIn};
use temor_panes::pane::{RenderHold, TransferEvent};
use temor_panes::pty::ChildExit;

use super::desk::Desk;
use super::model::Hub;
use super::offer::Heard as OfferHeard;
use crate::session::{BELL_MARK, Notice};
use crate::view::slot::{Slot, release_hold_later, repaint};
use temor_panes::Heard;

/// What a batch of events asks the window to do.
#[derive(Debug, Default)]
pub(super) struct Delivery {
    /// Panes to draw again.
    paint: BTreeSet<PaneId>,
    /// Panes whose chrome (title, folder, marks, the ended banner) changed.
    chrome: BTreeSet<PaneId>,
    /// Panes whose bell rang.
    bell: BTreeSet<PaneId>,
    /// Holds that started: the slot and the hold's generation, for its timer.
    holds: Vec<(Rc<RefCell<Slot>>, u64)>,
    /// What the workspace is told.
    asks: Vec<WorkspaceIn>,
    /// What the quick terminal is told (it ignores reports about other panes).
    quick: Vec<QuickIn>,
    /// What programs offered, sent or asked for through the terminal.
    transfers: Vec<(PaneId, TransferEvent)>,
}

impl Hub {
    /// Gives each event to its pane's session and gathers what that means for the window.
    pub(super) fn deliver(&mut self, batch: Vec<(PaneId, Heard)>) -> Delivery {
        let mut out = Delivery::default();
        // What the facts rule is told once the sessions are let go: it looks at the panes.
        let mut facts = Vec::new();
        for (pane, heard) in batch {
            let Some(slot) = self.slots.get(&pane).cloned() else {
                continue;
            };
            let mut guard = slot.borrow_mut();
            let Some(session) = guard.session.as_mut() else {
                continue;
            };
            // The pane's core took the event on the client's thread; the radar and the offer hear
            // what it left, and the session then takes that.
            if self.agents_observe(pane, &heard) {
                out.chrome.insert(pane);
            }
            let offered = OfferHeard::of(&heard);
            let notice = session.take(heard);
            self.history_heard(pane, &notice, session);
            if self.offer_heard(pane, offered, session) {
                out.chrome.insert(pane);
            }
            match notice {
                Notice::Repaint => {
                    out.paint.insert(pane);
                }
                Notice::Ended(exit) => {
                    let keep = self.keeps_ended_pane(pane, exit, session.prompts_seen() > 0);
                    self.run_ended(pane, exit);
                    self.ssh_ended(pane, exit);
                    out.paint.insert(pane);
                    out.chrome.insert(pane);
                    out.asks.push(WorkspaceIn::Activity(pane));
                    out.quick.push(QuickIn::ShellExited(pane));
                    // A shell that exited closes its pane (and the tab, with its last pane).
                    if !keep {
                        out.asks.push(WorkspaceIn::ClosePane(Some(pane)));
                    }
                }
                Notice::Hold(RenderHold::Held) => {
                    // Each hold has its own generation: a stale timer cannot end a newer one.
                    out.holds.push((Rc::clone(&slot), session.start_hold()));
                }
                Notice::Hold(RenderHold::Released) => {
                    session.release_hold();
                    out.paint.insert(pane);
                }
                Notice::Chrome => {
                    out.chrome.insert(pane);
                    if session.prompts_seen() > 0 {
                        facts.push((pane, FactsIn::Prompt));
                    }
                }
                Notice::Title(title) => {
                    out.chrome.insert(pane);
                    out.asks.extend(self.rename_for(pane, &title));
                }
                Notice::Cwd(path) => {
                    facts.push((pane, FactsIn::FolderChanged));
                    self.ssh_shell_started(pane);
                    out.chrome.insert(pane);
                    out.quick.push(QuickIn::Cwd {
                        pane,
                        cwd: Cwd(path.clone()),
                    });
                    out.asks.push(WorkspaceIn::Cwd {
                        pane,
                        cwd: Cwd(path),
                    });
                }
                Notice::CommandDone => {
                    facts.push((pane, FactsIn::CommandDone));
                    out.chrome.insert(pane);
                    out.asks.push(WorkspaceIn::Activity(pane));
                }
                Notice::Bell => {
                    out.bell.insert(pane);
                }
                Notice::Transfer(event) => out.transfers.push((pane, event)),
                Notice::Quiet => {}
            }
        }
        self.ssh_scan(&out.paint);
        for (pane, input) in facts {
            self.facts_in(pane, input);
        }
        out
    }

    /// Whether a pane whose program ended stays, with the banner that says so: a run tab (its
    /// output is what was asked for), a shell that ended before its first prompt (a missing
    /// program or an rc file that exits: a new tab would end the same way, and the banner shows
    /// why), a host session that never came up or whose connection dropped (Restart connects
    /// again), and the quick terminal (it closes itself).
    fn keeps_ended_pane(&self, pane: PaneId, exit: Option<ChildExit>, prompted: bool) -> bool {
        let host = self.ssh.panes.get(&pane).map(|host| host.phase);
        let dropped = host == Some(super::ssh::Phase::Up) && exit == Some(ChildExit::Exited(255));
        self.run_name(pane).is_some()
            || self.is_quick(pane)
            || !prompted
            || host.is_some_and(|phase| phase != super::ssh::Phase::Up)
            || dropped
    }

    /// The tab's title follows the program's title in the pane that has the keys in it.
    fn rename_for(&self, pane: PaneId, title: &str) -> Option<WorkspaceIn> {
        let tab = self.book.ws.tab_of(pane)?;
        let current = self.book.ws.tabs.tab(tab)?;
        (self.book.ws.focus_of(tab) == Some(pane)
            && !title.is_empty()
            && current.title.as_str() != title)
            .then(|| {
                WorkspaceIn::Tabs(TabsIn::Rename {
                    tab,
                    title: TabTitle(title.to_owned()),
                })
            })
    }
}

/// Hears the panes' events for as long as the window lives. Pending events are drained together
/// and the window repaints once for them.
pub fn spawn_event_loop(desk: Desk) {
    let Some(mut inbox) = desk.hub.0.borrow_mut().inbox.take() else {
        return;
    };
    spawn(async move {
        while let Some(first) = inbox.recv().await {
            let mut batch = vec![first];
            while let Ok(more) = inbox.try_recv() {
                batch.push(more);
            }
            let delivery = desk.hub.0.borrow_mut().deliver(batch);
            let Delivery {
                paint,
                chrome,
                bell,
                holds,
                asks,
                quick,
                transfers,
            } = delivery;
            let heard = !transfers.is_empty();
            let said = desk.hub.0.borrow_mut().transfers(transfers);
            for text in said {
                desk.say(text);
            }
            for (slot, generation) in holds {
                release_hold_later(slot, generation);
            }
            // Rows that moved around the room under a block need their slices drawn again.
            let mut moved = false;
            for pane in &paint {
                if let Some(slot) = desk.hub.slot(*pane) {
                    moved |= repaint(&slot);
                }
            }
            // A list of blocks grows with the output: it is drawn again for every paint.
            let listed = !paint.is_empty();
            let redraw = moved
                || listed
                || heard
                || !chrome.is_empty()
                || !bell.is_empty()
                || !asks.is_empty()
                || !quick.is_empty();
            for ask in asks {
                for text in desk.hub.act(ask) {
                    desk.say(text);
                }
            }
            for input in quick {
                desk.hub.0.borrow_mut().quick_input(input);
            }
            if redraw {
                desk.bump();
            }
            if !bell.is_empty() {
                // The bell's mark goes away on its own: a timer redraws once it has passed.
                let desk = desk.clone();
                spawn(async move {
                    sleep(BELL_MARK).await;
                    desk.bump();
                });
            }
        }
    });
}
