//! The shell offer in the window (PLAN P17): each pane's offer state, fed by the marks and the
//! output the pane's events carry, and by a timer for the rule that waits. The rule itself is
//! `temor_client::model::offer`; here the events are read, the screen is looked at, and a Yes types the
//! integration line into the pane. A host pane opened from temor's own list is never offered:
//! it already has the integration.

use std::hash::{DefaultHasher, Hash, Hasher};

use ds_settings::Store;
use temor_client::model::offer::{
    Answer, OfferIn, OfferList, OfferOut, OfferState, Program, Reply, offer_step,
};
use temor_client::model::vocab::ids::PaneId;
use temor_client::ssh::{InnerShell, ProbeReading, prompt_after_answer, read_probe};
use temor_panes::Heard as PaneHeard;

use super::desk::Desk;
use super::model::{Hub, HubRef};
use temor_pane_view::session::Session;
use temor_ui_base::launch::Launch;

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

/// The list the offer matches against, and where the choices about it are kept. What it keeps
/// of each pane is in the pane's record ([`PaneOffer`]).
pub(super) struct OfferHub {
    pub(super) list: OfferList,
    /// Where the "never for" choice is kept (`settings.toml`).
    pub(super) settings: Store,
}

impl OfferHub {
    pub(super) fn new(launch: &Launch) -> Self {
        Self {
            list: OfferList::new(&launch.settings.offer_add, &launch.settings.offer_remove),
            settings: launch.ambient.settings_store(),
        }
    }
}

/// The offer in one pane, kept in its record.
#[derive(Debug, Default)]
pub(super) struct PaneOffer {
    pub(super) state: OfferState,
    /// A hash of the last line on the screen while it is watched: output counts only when that
    /// line changes, so a repaint or a resize (the offer's own bar shrinks the grid) is not output.
    line: Option<u64>,
    /// The shell the pane named, kept from the tick that heard it to the load.
    pub(super) shell: Option<InnerShell>,
    /// A hash of the whole screen at the last tick while it is probed: the integration is typed
    /// once the shell's next prompt is on a screen that did not change between two ticks.
    screen: Option<u64>,
}

impl PaneOffer {
    /// Whether output on the screen counts: the offer waits on it or shows.
    fn watched(&self) -> bool {
        matches!(
            self.state,
            OfferState::Watching { .. } | OfferState::Asking { .. }
        )
    }
}

impl Hub {
    /// Gives the offer what one event from `pane` means, after its session took the event. True
    /// when the window must be drawn again.
    pub(super) fn offer_heard(&mut self, pane: PaneId, heard: Heard, session: &Session) -> bool {
        if self.host_of(pane).is_some() {
            return false;
        }
        let Some(record) = self.panes.get_mut(pane) else {
            return false;
        };
        let at = session.now();
        let input = match heard {
            Heard::Output => {
                if !record.offer.watched() {
                    return false;
                }
                let line = line_hash(&session.last_line());
                if record.offer.line.replace(line) == Some(line) {
                    return false;
                }
                OfferIn::Output { at }
            }
            Heard::Command => OfferIn::Running {
                now: session.running_listed(&self.offer.list),
                at,
            },
            Heard::Other => return false,
        };
        let started = matches!(input, OfferIn::Running { now: Some(_), .. });
        let redraw = self.offer_feed(pane, input).0;
        if started && let Some(record) = self.panes.get_mut(pane) {
            record.offer.line = Some(line_hash(&session.last_line()));
        }
        redraw
    }

    /// One timer tick: panes whose listed command has run a while may now get the offer, and
    /// panes that were asked which shell they run may have answered. Returns whether the window
    /// must be drawn again.
    pub(super) fn offer_tick(&mut self) -> bool {
        let waiting: Vec<PaneId> = self
            .panes
            .iter()
            .filter(|(_, record)| {
                matches!(
                    record.offer.state,
                    OfferState::Watching { .. } | OfferState::Probing { .. }
                )
            })
            .map(|(pane, _)| pane)
            .collect();
        let mut redraw = false;
        for pane in waiting {
            let Some(record) = self.panes.get_mut(pane) else {
                continue;
            };
            let Some(slot) = record.slot.clone() else {
                continue;
            };
            let offer = &mut record.offer;
            let probing = matches!(offer.state, OfferState::Probing { .. });
            let input = match slot.borrow().session.as_ref() {
                Some(session) if probing => {
                    let screen = session.screen_text();
                    let hash = line_hash(&screen);
                    let still = offer.screen.replace(hash) == Some(hash);
                    let answer = match read_probe(&screen) {
                        ProbeReading::Silent => Answer::Silent,
                        ProbeReading::Known(shell) => {
                            offer.shell = Some(shell);
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
            redraw |= seen;
            let said = self.offer_outs(pane, outs);
            self.notices.extend(said);
        }
        redraw
    }

    pub(super) fn offer_feed(&mut self, pane: PaneId, input: OfferIn) -> (bool, Vec<OfferOut>) {
        let Some(record) = self.panes.get_mut(pane) else {
            return (false, Vec::new());
        };
        let offer = &mut record.offer;
        let (next, outs) = offer_step(std::mem::take(&mut offer.state), input);
        if !matches!(next, OfferState::Probing { .. }) {
            offer.screen = None;
        }
        if next == OfferState::Idle {
            offer.line = None;
        }
        offer.state = next;
        (outs.contains(&OfferOut::Redraw), outs)
    }

    /// The listed command being offered in `pane`, while its offer shows.
    pub fn offer_asking(&self, pane: PaneId) -> Option<String> {
        self.panes
            .get(pane)?
            .offer
            .state
            .asking()
            .map(Program::to_string)
    }
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
        self.tell(self.hub.edit(|hub| hub.offer_reply(pane, reply)));
    }
}
