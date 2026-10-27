//! Hearing the panes on a thread of the client's (PLAN §2b, P24c). Every pane's events arrive on
//! one channel tagged with its id; the `temor-hear` thread gives each to its pane's core
//! ([`PaneCore::hear`]) and sends the window what is left ([`Heard`]). The window never parses a
//! mark or reads a command's text; it only shows what the core says.
//!
//! A pane's core is shared with the window ([`Shared`]): the window paints from it and types into
//! it, each under the core's lock. Locks are taken in one order, the panes' table before a core
//! and a core before its terminal, so the thread and the window never wait on each other in a
//! ring.
//!
//! A pane is [registered](Hearing::register) once its core exists; what it said before that (its
//! first screen, the shell's first marks) waits in the table and is heard then, in order.

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use parking_lot::Mutex;
use temor_core::vocab::ids::PaneId;
use temor_pane::PaneEvent;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::{Heard, PaneCore};

/// A pane's core, as the window and the client's thread both hold it.
pub type Shared = Arc<Mutex<PaneCore>>;

/// A core to share between the window and the client's thread.
pub fn share(core: PaneCore) -> Shared {
    Arc::new(Mutex::new(core))
}

/// A pane's core, locked.
pub type Locked<'a> = parking_lot::MutexGuard<'a, PaneCore>;

/// The most events a pane that is not registered yet keeps waiting; older ones are dropped.
const WAITING_MAX: usize = 4096;

/// The panes the thread hears, and what unregistered ones said.
#[derive(Default)]
struct Panes {
    cores: BTreeMap<PaneId, Weak<Mutex<PaneCore>>>,
    waiting: BTreeMap<PaneId, Vec<PaneEvent>>,
}

/// Where panes' events go, and how a pane is made known to the thread that hears them. Clones
/// share the thread.
#[derive(Clone)]
pub struct Hearing {
    events: UnboundedSender<(PaneId, PaneEvent)>,
    panes: Arc<Mutex<Panes>>,
    window: UnboundedSender<(PaneId, Heard)>,
}

impl std::fmt::Debug for Hearing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hearing").finish_non_exhaustive()
    }
}

impl Hearing {
    /// Starts the thread. What each pane's core leaves for the window comes out of the receiver.
    pub fn start() -> std::io::Result<(Self, UnboundedReceiver<(PaneId, Heard)>)> {
        let (events, mut inbox) = unbounded_channel::<(PaneId, PaneEvent)>();
        let (window, heard) = unbounded_channel();
        let hearing = Self {
            events,
            panes: Arc::default(),
            window,
        };
        let (panes, onward) = (Arc::clone(&hearing.panes), hearing.window.clone());
        std::thread::Builder::new()
            .name("temor-hear".to_owned())
            .spawn(move || {
                while let Some(first) = inbox.blocking_recv() {
                    let mut panes = panes.lock();
                    hear(&mut panes, &onward, first);
                    while let Ok(more) = inbox.try_recv() {
                        hear(&mut panes, &onward, more);
                    }
                }
            })?;
        Ok((hearing, heard))
    }

    /// The sender a pane's events go to, tagged with the pane.
    pub fn events(&self) -> UnboundedSender<(PaneId, PaneEvent)> {
        self.events.clone()
    }

    /// Makes `pane`'s core known to the thread, and hears what the pane said before.
    pub fn register(&self, pane: PaneId, core: &Shared) {
        let mut panes = self.panes.lock();
        panes.cores.insert(pane, Arc::downgrade(core));
        for event in panes.waiting.remove(&pane).unwrap_or_default() {
            let heard = core.lock().hear(event);
            let _ = self.window.send((pane, heard));
        }
    }
}

/// Gives one event to its pane's core. A pane whose core is gone hears nothing more (its entry
/// stays, so a late event is not taken for a new pane's); one not registered yet keeps the event
/// for when it is.
fn hear(
    panes: &mut Panes,
    window: &UnboundedSender<(PaneId, Heard)>,
    (pane, event): (PaneId, PaneEvent),
) {
    let core = panes.cores.get(&pane).map(Weak::upgrade);
    match core {
        Some(Some(core)) => {
            let heard = core.lock().hear(event);
            let _ = window.send((pane, heard));
        }
        Some(None) => {}
        None => {
            let waiting = panes.waiting.entry(pane).or_default();
            if waiting.len() >= WAITING_MAX {
                waiting.remove(0);
            }
            waiting.push(event);
        }
    }
}
