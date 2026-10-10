//! Hearing the panes. Every pane's events arrive on one channel tagged with its id; the window
//! drains it, gives each event to its pane's session, and reacts to what the sessions report
//! (repaint, redraw the chrome, note a finished command or a new folder in the workspace).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use temor_core::quick::QuickIn;
use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::{Cwd, TabTitle};
use temor_core::workspace::{TabsIn, WorkspaceIn};
use temor_pane::{Hold, PaneEvent};

use super::desk::Desk;
use super::model::Hub;
use crate::session::{BELL_MARK, Notice};
use crate::view::slot::{Slot, release_hold_later, repaint};

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
}

impl Hub {
    /// Gives each event to its pane's session and gathers what that means for the window.
    pub(super) fn deliver(&mut self, batch: Vec<(PaneId, PaneEvent)>) -> Delivery {
        let mut out = Delivery::default();
        for (pane, event) in batch {
            let Some(slot) = self.slots.get(&pane).cloned() else {
                continue;
            };
            let mut guard = slot.borrow_mut();
            let Some(session) = guard.session.as_mut() else {
                continue;
            };
            match session.absorb(event) {
                Notice::Repaint => {
                    out.paint.insert(pane);
                }
                Notice::Ended(_) => {
                    out.paint.insert(pane);
                    out.chrome.insert(pane);
                    out.asks.push(WorkspaceIn::Activity(pane));
                    out.quick.push(QuickIn::ShellExited(pane));
                }
                Notice::Hold(Hold::Held) => {
                    // Each hold has its own generation: a stale timer cannot end a newer one.
                    out.holds.push((Rc::clone(&slot), session.start_hold()));
                }
                Notice::Hold(Hold::Released) => {
                    session.release_hold();
                    out.paint.insert(pane);
                }
                Notice::Chrome => {
                    out.chrome.insert(pane);
                }
                Notice::Title(title) => {
                    out.chrome.insert(pane);
                    out.asks.extend(self.rename_for(pane, &title));
                }
                Notice::Cwd(path) => {
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
                    out.chrome.insert(pane);
                    out.asks.push(WorkspaceIn::Activity(pane));
                }
                Notice::Bell => {
                    out.bell.insert(pane);
                }
                Notice::Quiet => {}
            }
        }
        out
    }

    /// The tab's title follows the program's title in the pane that has the keys in it.
    fn rename_for(&self, pane: PaneId, title: &str) -> Option<WorkspaceIn> {
        let tab = self.ws.tab_of(pane)?;
        let current = self.ws.tabs.tab(tab)?;
        (self.ws.focus_of(tab) == Some(pane)
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
            } = delivery;
            for (slot, generation) in holds {
                release_hold_later(slot, generation);
            }
            for pane in &paint {
                if let Some(slot) = desk.hub.slot(*pane) {
                    repaint(&slot);
                }
            }
            let redraw =
                !chrome.is_empty() || !bell.is_empty() || !asks.is_empty() || !quick.is_empty();
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
