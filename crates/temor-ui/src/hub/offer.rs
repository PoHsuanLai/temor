//! The shell offer in the window (PLAN P17): each pane's offer state, fed by the marks and the
//! output the pane's events carry, and by a timer for the rule that waits. The rule itself is
//! `temor_core::offer`; here the events are read, the screen is looked at, and a Yes types the
//! integration line into the pane. A host pane opened from temor's own list is never offered:
//! it already has the integration.

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use ds_settings::Store;
use temor_core::offer::{
    Answer, OfferIn, OfferList, OfferOut, OfferState, Program, Reply, offer_step,
};
use temor_core::vocab::ids::PaneId;
use temor_panes::Heard as PaneHeard;
use temor_ssh::{InnerShell, ProbeReading, prompt_after_answer, read_probe};

use super::desk::Desk;
use super::model::{Hub, HubRef};
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
    pub(super) fn of(heard: &PaneHeard) -> Self {
        match heard {
            PaneHeard::Repaint => Heard::Output,
            PaneHeard::Marked | PaneHeard::CommandDone | PaneHeard::Reset | PaneHeard::Ended(_) => {
                Heard::Command
            }
            _ => Heard::Other,
        }
    }
}

fn line_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// The offer's state for every pane, and the list it matches against.
pub(super) struct OfferHub {
    pub(super) list: OfferList,
    pub(super) states: BTreeMap<PaneId, OfferState>,
    /// A hash of the last line on each watched pane's screen: output counts only when that
    /// line changes, so a repaint or a resize (the offer's own bar shrinks the grid) is not output.
    pub(super) lines: BTreeMap<PaneId, u64>,
    /// The shell each probed pane named, kept from the tick that heard it to the load.
    pub(super) shells: BTreeMap<PaneId, InnerShell>,
    /// A hash of each probed pane's whole screen at the last tick: the integration is typed
    /// once the shell's next prompt is on a screen that did not change between two ticks.
    pub(super) screens: BTreeMap<PaneId, u64>,
    /// Where the "never for" choice is kept (`settings.toml`).
    pub(super) settings: Store,
}

impl OfferHub {
    pub(super) fn new(launch: &Launch) -> Self {
        Self {
            list: OfferList::new(&launch.settings.offer_add, &launch.settings.offer_remove),
            states: BTreeMap::new(),
            lines: BTreeMap::new(),
            shells: BTreeMap::new(),
            screens: BTreeMap::new(),
            settings: launch.ambient.settings_store(),
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

    /// One timer tick: panes whose listed command has run a while may now get the offer, and
    /// panes that were asked which shell they run may have answered. Returns what the person
    /// should be told.
    pub(super) fn offer_tick(&mut self) -> (OfferRedraw, Vec<String>) {
        let waiting: Vec<PaneId> = self
            .offer
            .states
            .iter()
            .filter(|(_, state)| {
                matches!(
                    state,
                    OfferState::Watching { .. } | OfferState::Probing { .. }
                )
            })
            .map(|(pane, _)| *pane)
            .collect();
        let mut redraw = OfferRedraw::No;
        let mut notes = Vec::new();
        for pane in waiting {
            let Some(slot) = self.slots.get(&pane).cloned() else {
                continue;
            };
            let probing = matches!(
                self.offer.states.get(&pane),
                Some(OfferState::Probing { .. })
            );
            let input = match slot.borrow().session.as_ref() {
                Some(session) if probing => {
                    let screen = session.screen_text();
                    let hash = line_hash(&screen);
                    let still = self.offer.screens.insert(pane, hash) == Some(hash);
                    let answer = match read_probe(&screen) {
                        ProbeReading::Silent => Answer::Silent,
                        ProbeReading::Known(shell) => {
                            self.offer.shells.insert(pane, shell);
                            match still && prompt_after_answer(&screen) {
                                true => Answer::Known,
                                false => Answer::Named,
                            }
                        }
                        ProbeReading::Other => Answer::Other,
                    };
                    OfferIn::Heard {
                        at: session.now(),
                        answer,
                    }
                }
                Some(session) => OfferIn::Tick {
                    at: session.now(),
                    screen: session.screen_kind(),
                    line: session.last_line_kind(),
                },
                None => continue,
            };
            let (seen, outs) = self.offer_feed(pane, input);
            if seen == OfferRedraw::Yes {
                redraw = OfferRedraw::Yes;
            }
            notes.extend(self.offer_outs(pane, outs));
        }
        (redraw, notes)
    }

    pub(super) fn offer_feed(
        &mut self,
        pane: PaneId,
        input: OfferIn,
    ) -> (OfferRedraw, Vec<OfferOut>) {
        let state = self.offer.states.remove(&pane).unwrap_or_default();
        let (next, outs) = offer_step(state, input);
        if !matches!(next, OfferState::Probing { .. }) {
            self.offer.screens.remove(&pane);
        }
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
}

/// Looks at the waiting rule for as long as the window lives.
pub fn spawn_offer_loop(desk: Desk) {
    spawn(async move {
        loop {
            sleep(TICK).await;
            let (redraw, notes) = desk.hub.0.borrow_mut().offer_tick();
            for note in notes {
                desk.say(note);
            }
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
