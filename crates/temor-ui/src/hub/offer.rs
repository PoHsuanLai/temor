//! The shell offer in the window (PLAN P17): each pane's offer state, fed by the marks and the
//! output the pane's events carry, and by a timer for the rule that waits. The rule itself is
//! `temor_core::offer`; here the events are read, the screen is looked at, and a Yes types the
//! integration line into the pane. A host pane opened from temor's own list is never offered:
//! it already has the integration.

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use temor_core::offer::{
    Line, OfferIn, OfferList, OfferOut, OfferState, Program, Reply, Screen, offer_step,
};
use temor_core::vocab::ids::PaneId;
use temor_pane::PaneEvent;

use super::desk::Desk;
use super::model::{Hub, HubRef};
use crate::edge::offer::{load_line, remember_never};
use crate::session::Session;
use crate::view::launch::Launch;

/// How often the waiting rule is looked at.
const TICK: Duration = Duration::from_millis(250);

/// Whether the window must be drawn again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfferRedraw {
    Yes,
    No,
}

/// What an event from a pane means to the offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Heard {
    /// The screen changed: output arrived.
    Output,
    /// A mark, a reset or the end of the shell: the running command may have changed.
    Command,
    /// Nothing for the offer.
    Other,
}

impl Heard {
    pub(super) fn of(event: &PaneEvent) -> Self {
        match event {
            PaneEvent::Dirty => Heard::Output,
            PaneEvent::Anchor { .. } | PaneEvent::Reset | PaneEvent::Exited(_) => Heard::Command,
            _ => Heard::Other,
        }
    }
}

fn line_hash(line: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    line.hash(&mut hasher);
    hasher.finish()
}

/// The offer's state for every pane, and the list it matches against.
pub(super) struct OfferHub {
    list: OfferList,
    states: BTreeMap<PaneId, OfferState>,
    /// A hash of the last line on each watched pane's screen: output counts only when that
    /// line changes, so a repaint or a resize (the offer's own bar shrinks the grid) is not output.
    lines: BTreeMap<PaneId, u64>,
    settings_path: PathBuf,
}

impl OfferHub {
    pub(super) fn new(launch: &Launch) -> Self {
        Self {
            list: OfferList::new(&launch.settings.offer_add, &launch.settings.offer_remove),
            states: BTreeMap::new(),
            lines: BTreeMap::new(),
            settings_path: launch.ambient.settings_path(),
        }
    }
}

impl Hub {
    /// Gives the offer what one event from `pane` means, after its session took the event.
    pub(super) fn offer_heard(
        &mut self,
        pane: PaneId,
        heard: Heard,
        session: &Session,
    ) -> OfferRedraw {
        if self.host_of(pane).is_some() {
            return OfferRedraw::No;
        }
        let at = session.now();
        let input = match heard {
            Heard::Output => {
                let watched = self.offer.states.get(&pane).is_some_and(|state| {
                    matches!(
                        state,
                        OfferState::Watching { .. } | OfferState::Asking { .. }
                    )
                });
                if !watched {
                    return OfferRedraw::No;
                }
                let line = line_hash(&session.last_line());
                if self.offer.lines.insert(pane, line) == Some(line) {
                    return OfferRedraw::No;
                }
                OfferIn::Output { at }
            }
            Heard::Command => OfferIn::Running {
                now: session.running_listed(&self.offer.list),
                at,
            },
            Heard::Other => return OfferRedraw::No,
        };
        let started = matches!(input, OfferIn::Running { now: Some(_), .. });
        let redraw = self.offer_feed(pane, input).0;
        if started {
            self.offer
                .lines
                .insert(pane, line_hash(&session.last_line()));
        }
        redraw
    }

    /// One timer tick: panes whose listed command has run a while may now get the offer.
    pub(super) fn offer_tick(&mut self) -> OfferRedraw {
        let waiting: Vec<PaneId> = self
            .offer
            .states
            .iter()
            .filter(|(_, state)| matches!(state, OfferState::Watching { .. }))
            .map(|(pane, _)| *pane)
            .collect();
        let mut redraw = OfferRedraw::No;
        for pane in waiting {
            let Some(slot) = self.slots.get(&pane).cloned() else {
                continue;
            };
            let input = match slot.borrow().session.as_ref() {
                Some(session) => OfferIn::Tick {
                    at: session.now(),
                    screen: session.screen_kind(),
                    line: session.last_line_kind(),
                },
                None => continue,
            };
            if self.offer_feed(pane, input).0 == OfferRedraw::Yes {
                redraw = OfferRedraw::Yes;
            }
        }
        redraw
    }

    fn offer_feed(&mut self, pane: PaneId, input: OfferIn) -> (OfferRedraw, Vec<OfferOut>) {
        let state = self.offer.states.remove(&pane).unwrap_or_default();
        let (next, outs) = offer_step(state, input);
        match next {
            OfferState::Idle => {
                self.offer.lines.remove(&pane);
            }
            next => {
                self.offer.states.insert(pane, next);
            }
        }
        let redraw = match outs.contains(&OfferOut::Redraw) {
            true => OfferRedraw::Yes,
            false => OfferRedraw::No,
        };
        (redraw, outs)
    }

    /// The listed command being offered in `pane`, while its offer shows.
    pub fn offer_asking(&self, pane: PaneId) -> Option<String> {
        self.offer
            .states
            .get(&pane)
            .and_then(OfferState::asking)
            .map(Program::to_string)
    }

    /// The person answered the offer in `pane`. A Yes types the integration line into it;
    /// returns what the person should be told.
    pub fn offer_reply(&mut self, pane: PaneId, reply: Reply) -> Vec<String> {
        let (_, outs) = self.offer_feed(pane, OfferIn::Reply(reply));
        let mut notes = Vec::new();
        for out in outs {
            let note = match out {
                OfferOut::Redraw => None,
                OfferOut::Load => self.offer_load(pane),
                OfferOut::Never(program) => self.offer_never(&program),
            };
            notes.extend(note);
        }
        notes
    }

    /// Types the line that loads the integration into `pane`, unless the program is asking
    /// something right now: the line would answer it.
    fn offer_load(&self, pane: PaneId) -> Option<String> {
        let slot = self.slots.get(&pane)?;
        let guard = slot.borrow();
        let session = guard.session.as_ref()?;
        if session.last_line_kind() == Line::Asking || session.screen_kind() == Screen::Alternate {
            return Some("Not set up: the program is waiting for an answer.".to_owned());
        }
        match load_line() {
            Ok(line) => {
                session.pane().send(line.as_str().as_bytes());
                None
            }
            Err(_) => Some("Could not set up blocks in this shell.".to_owned()),
        }
    }

    /// Takes `program` off the list, now and in the settings file.
    fn offer_never(&mut self, program: &Program) -> Option<String> {
        self.offer.list.remove(program);
        remember_never(&self.offer.settings_path, program)
            .err()
            .map(|_| "Could not save this choice.".to_owned())
    }
}

/// Looks at the waiting rule for as long as the window lives.
pub fn spawn_offer_loop(desk: Desk) {
    spawn(async move {
        loop {
            sleep(TICK).await;
            let redraw = desk.hub.0.borrow_mut().offer_tick();
            if redraw == OfferRedraw::Yes {
                desk.bump();
            }
        }
    });
}

impl HubRef {
    /// The listed command being offered in `pane`, while its offer shows.
    pub fn offer_asking(&self, pane: PaneId) -> Option<String> {
        self.0.borrow().offer_asking(pane)
    }
}

impl Desk {
    /// The person's answer to the offer in `pane`.
    pub fn offer_answer(&self, pane: PaneId, reply: Reply) {
        let notes = self.hub.0.borrow_mut().offer_reply(pane, reply);
        for note in notes {
            self.say(note);
        }
        self.bump();
    }
}
