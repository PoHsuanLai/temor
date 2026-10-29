//! The ⌘-hover peek on a terminal pane (PLAN §10, D13): the pane's pointer and key events feed the
//! pure step (`hub::peek_step`), whose answer shows or hides the card. The card takes neither the
//! keyboard nor the pointer; it is drawn over the terminal and lets every event through. A pane
//! on another computer shows one only when its host has the helper (the file is fetched first).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use ds::edit::raw_key::RawKey;
use ds::host::measure::{RectProbe, use_rect};
use ds::host::signals::HostSignals;
use temor_client::model::vocab::ids::PaneId;
use temor_panes::vt::SurfacePoint;

use super::peek_card::PeekCard;
use super::peek_keys::{KeySays, says};
use super::slot::Slot;
use super::window_help::{device_at, pane_logical};
use crate::edge::peek_mail::Taken;
use crate::hub::Desk;
use crate::hub::peek_step::{Command, Peek, PeekEffect, PeekEvent, Under, step};
use crate::input::vt_mods;

/// How often the window looks for the card the worker is making.
const POLL: Duration = Duration::from_millis(40);
/// How many looks before giving up on a card (about five seconds, past the worker's own limit).
const POLLS: u32 = 125;

/// What the pane's view wires in.
#[derive(Clone)]
pub struct PeekHandles {
    /// The pointer moved over the grid.
    pub moved: Rc<dyn Fn(PointerEvent)>,
    /// The pointer left the grid or pressed.
    pub gone: Rc<dyn Fn()>,
    /// A key event the pane heard.
    pub key: Rc<dyn Fn(&RawKey)>,
    /// The card to draw, once the worker has made it.
    pub card: Signal<Option<PeekCard>>,
    /// The drawn card's size, which places it.
    pub measure: RectProbe,
}

impl std::fmt::Debug for PeekHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeekHandles").finish_non_exhaustive()
    }
}

/// Where the pointer last was.
#[derive(Clone, Copy)]
struct Pointed {
    at: SurfacePoint,
    x: f32,
    y: f32,
}

/// What the peek remembers between events.
#[derive(Default)]
struct Memo {
    peek: Peek,
    last: Option<Pointed>,
}

/// Everything an event needs to be fed to the step.
#[derive(Clone)]
struct Feeder {
    pane: PaneId,
    slot: Rc<RefCell<Slot>>,
    desk: Desk,
    rect: RectProbe,
    memo: Rc<RefCell<Memo>>,
    card: Signal<Option<PeekCard>>,
}

impl Feeder {
    /// The file under the pointer's last place, while ⌘ is held.
    fn under(&self, command: Command) -> Option<Under> {
        if command == Command::Up {
            return None;
        }
        let last = self.memo.borrow().last?;
        let file = self.slot.borrow().session.as_ref()?.file_under(last.at)?;
        Some(Under {
            file,
            x: last.x,
            y: last.y,
        })
    }

    /// Runs the step on `event` and carries out its answer.
    fn feed(&self, event: PeekEvent) {
        let effect = {
            let mut memo = self.memo.borrow_mut();
            let (next, effect) = step(&memo.peek, event);
            memo.peek = next;
            effect
        };
        match effect {
            PeekEffect::Nothing => {}
            PeekEffect::Hide => self.hide(),
            PeekEffect::Show(under) => self.show(under),
        }
    }

    fn hide(&self) {
        self.desk.hub.peek_cancel();
        let mut card = self.card;
        if card.peek().is_some() {
            card.set(None);
        }
    }

    fn show(&self, under: Under) {
        self.hide();
        let Some(room) = self.rect.rect().map(|r| (r.size.width.0, r.size.height.0)) else {
            return;
        };
        let Some(ticket) = self.desk.hub.peek_ask(self.pane, &under.file) else {
            return;
        };
        let (hub, mut card) = (self.desk.hub.clone(), self.card);
        spawn(async move {
            for _ in 0..POLLS {
                match hub.peek_take(ticket) {
                    Taken::Ready(peeked) => {
                        card.set(Some(PeekCard {
                            peeked,
                            pointer: (under.x, under.y),
                            room,
                        }));
                        return;
                    }
                    Taken::Stale => return,
                    Taken::Waiting => sleep(POLL).await,
                }
            }
        });
    }
}

/// The pane's peek: call it once, in the pane's component, and wire the handles in.
pub fn use_peek(
    pane: PaneId,
    slot: Rc<RefCell<Slot>>,
    desk: Desk,
    rect: RectProbe,
    signals: Option<HostSignals>,
) -> PeekHandles {
    let card = use_signal(|| None::<PeekCard>);
    let measure = use_rect();
    let memo = use_hook(|| Rc::new(RefCell::new(Memo::default())));
    let feeder = Feeder {
        pane,
        slot,
        desk,
        rect,
        memo,
        card,
    };
    let moved = {
        let feeder = feeder.clone();
        move |event: PointerEvent| {
            let client = event.client_coordinates();
            let (x, y) = pane_logical(client, &feeder.rect);
            feeder.memo.borrow_mut().last = Some(Pointed {
                at: device_at(client, &feeder.rect, signals),
                x: x as f32,
                y: y as f32,
            });
            let command = Command::of(vt_mods(event.modifiers()));
            let under = feeder.under(command);
            feeder.feed(PeekEvent::Looked { command, under });
        }
    };
    let gone = {
        let feeder = feeder.clone();
        move || feeder.feed(PeekEvent::Gone)
    };
    let key = move |raw: &RawKey| match says(raw) {
        KeySays::Look(command) => {
            let under = feeder.under(command);
            feeder.feed(PeekEvent::Looked { command, under });
        }
        KeySays::Typed => {
            let held = Command::of(vt_mods(raw.modifiers));
            let over = feeder.under(held).map(|under| under.file);
            feeder.feed(PeekEvent::Typed { over });
        }
    };
    PeekHandles {
        moved: Rc::new(moved),
        gone: Rc::new(gone),
        key: Rc::new(key),
        card,
        measure,
    }
}
