//! Live cards in the window: a running program moved to a terminal of its own (a local daemon
//! session), shown in a pane beside the one it ran in, with a head of its own: its command, how
//! it stands, and icons to size it, stop it and put it away. The rules are `card_step`; here its
//! outputs are carried out on the workspace, the panes and the daemon.

use std::collections::BTreeMap;

use temor_client::model::session::{CardIn, CardOut, CardSize, Cards, Life, SurfaceId, card_step};
use temor_client::model::split::{Direction, Share};
use temor_client::model::vocab::ids::{BlockId, PaneId};
use temor_client::model::workspace::WorkspaceIn;
use temor_client::remote::ids::SessionId;

use super::desk::Desk;
use super::model::{Hub, HubRef};

/// How long a program has to stop after an interrupt before it is hung up.
pub const GRACE: std::time::Duration = std::time::Duration::from_secs(3);

/// [`GRACE`] in whole seconds, for the words that speak of it.
pub fn live_cards_grace_secs() -> u64 {
    GRACE.as_secs()
}

/// The live cards of the window.
#[derive(Debug, Default)]
pub struct LiveCards {
    /// The cards of each pane, by the pane their commands ran in.
    cards: BTreeMap<PaneId, Cards>,
    /// Where each card is shown: the pane, by the pane and block it came from.
    shown: BTreeMap<(PaneId, BlockId), PaneId>,
    /// Each card's command line.
    commands: BTreeMap<(PaneId, BlockId), String>,
    /// The cards whose grace runs, and when it is over.
    grace: BTreeMap<(PaneId, BlockId), std::time::Instant>,
}

/// What a live card's head says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveHead {
    pub command: String,
    pub size: CardSize,
    pub life: Life,
}

/// An icon of a live card's head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveIcon {
    /// The next size.
    Size,
    /// The whole tab, or back.
    Full,
    Stop,
    /// Put an ended card away.
    Dismiss,
}

impl LiveCards {
    /// The card shown in `pane`: the pane and block it came from.
    fn card_in(&self, pane: PaneId) -> Option<(PaneId, BlockId)> {
        self.shown
            .iter()
            .find(|(_, shown)| **shown == pane)
            .map(|(key, _)| *key)
    }
}

impl Hub {
    /// Whether the command running in `pane` can be moved to a card: the pane is a daemon
    /// session and a command runs in it.
    pub fn can_detach(&self, pane: PaneId) -> bool {
        self.remote.local.contains_key(&pane)
            && self
                .remote
                .panes
                .get(&pane)
                .is_some_and(|p| p.session.is_some())
            && self
                .live(pane, |session| session.running_block())
                .flatten()
                .is_some()
    }

    /// Moves the command running in `pane` to a live card.
    pub fn detach_block(&mut self, pane: PaneId) {
        let Some(id) = self.live(pane, |session| session.running_block()).flatten() else {
            return;
        };
        self.card(pane, CardIn::Detach(id));
    }

    /// What the head of the live card shown in `pane` says, when it shows one.
    pub fn live_head(&self, pane: PaneId) -> Option<LiveHead> {
        let (origin, id) = self.live_cards.card_in(pane)?;
        let surface = self.live_cards.cards.get(&origin)?.get(id)?;
        Some(LiveHead {
            command: self
                .live_cards
                .commands
                .get(&(origin, id))
                .cloned()
                .unwrap_or_default(),
            size: surface.size,
            life: surface.life,
        })
    }

    /// An icon of the head of the live card shown in `pane` was pressed.
    pub fn live_icon(&mut self, pane: PaneId, icon: LiveIcon) {
        let Some((origin, id)) = self.live_cards.card_in(pane) else {
            return;
        };
        let size = self
            .live_cards
            .cards
            .get(&origin)
            .and_then(|cards| cards.get(id))
            .map(|surface| surface.size);
        let input = match (icon, size) {
            (LiveIcon::Size, _) => CardIn::CycleSize(id),
            (LiveIcon::Full, Some(CardSize::Full)) => CardIn::SetSize {
                id,
                size: CardSize::Normal,
            },
            (LiveIcon::Full, _) => CardIn::SetSize {
                id,
                size: CardSize::Full,
            },
            (LiveIcon::Stop, _) => CardIn::Terminate(id),
            (LiveIcon::Dismiss, _) => CardIn::Dismiss(id),
        };
        self.card(origin, input);
    }

    /// The graces that are over end: their programs are hung up.
    pub fn live_tick(&mut self, now: std::time::Instant) -> bool {
        let over: Vec<_> = self
            .live_cards
            .grace
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(key, _)| *key)
            .collect();
        for (origin, id) in &over {
            self.live_cards.grace.remove(&(*origin, *id));
            self.card(*origin, CardIn::GraceOver(*id));
        }
        !over.is_empty()
    }

    /// The daemon session shown in `pane` ended: a live card shown there keeps its last frame.
    pub(super) fn live_ended(&mut self, pane: PaneId, exit: Option<i32>) {
        if let Some((origin, id)) = self.live_cards.card_in(pane) {
            self.live_cards.grace.remove(&(origin, id));
            self.card(origin, CardIn::Ended { id, exit });
        }
    }

    /// Whether `pane` shows a live card: it stays when its program ends, until put away.
    pub(super) fn shows_live_card(&self, pane: PaneId) -> bool {
        self.live_cards.card_in(pane).is_some()
    }

    /// The pane showing a live card was closed: the card goes with it.
    pub(super) fn live_closed(&mut self, pane: PaneId) {
        let Some(key) = self.live_cards.card_in(pane) else {
            return;
        };
        self.live_cards.shown.remove(&key);
        let (origin, id) = key;
        self.card(origin, CardIn::Ended { id, exit: None });
        self.card(origin, CardIn::Dismiss(id));
    }

    /// Steps the cards of `origin` and carries out what the step says.
    fn card(&mut self, origin: PaneId, input: CardIn) {
        let cards = self.live_cards.cards.remove(&origin).unwrap_or_default();
        let (cards, outs) = card_step(cards, input);
        if !cards.is_empty() {
            self.live_cards.cards.insert(origin, cards);
        }
        for out in outs {
            self.carry_card(origin, out);
        }
    }

    fn carry_card(&mut self, origin: PaneId, out: CardOut) {
        match out {
            CardOut::Detach(id) => self.card_detach(origin, id),
            CardOut::Resize { session, size } => self.card_resize(origin, session, size),
            CardOut::Interrupt(session) => {
                if let Some(pane) = self.card_pane(origin, session) {
                    self.live(pane, |s| s.send_bytes(b"\x03"));
                }
            }
            CardOut::StartGrace(id) => {
                let over = std::time::Instant::now() + GRACE;
                self.live_cards.grace.insert((origin, id), over);
            }
            CardOut::HangUp(session) => self.local_hang_up(SessionId(session.0)),
            CardOut::Redraw(id) => {
                // A card put away takes its pane with it.
                let gone = self
                    .live_cards
                    .cards
                    .get(&origin)
                    .is_none_or(|cards| cards.get(id).is_none());
                if gone && let Some(pane) = self.live_cards.shown.remove(&(origin, id)) {
                    self.live_cards.commands.remove(&(origin, id));
                    self.act(WorkspaceIn::ClosePane(Some(pane)));
                }
            }
            // The pane of an ended session keeps its last frame by itself; this window has no
            // mini window to open.
            _ => {}
        }
    }

    /// Moves the program to a session of its own in a new pane beside `origin`.
    fn card_detach(&mut self, origin: PaneId, id: BlockId) {
        let command = self
            .live(origin, |s| s.running_command().map(|r| r.command))
            .flatten()
            .unwrap_or_default();
        // The card's pane shows the screen the program ran on: the commands before it are told
        // how they ended from what the pane they ran in kept.
        let history = self
            .live(origin, |s| s.checkpoint().history)
            .unwrap_or_default();
        self.act(WorkspaceIn::Focus(origin));
        if self.detach_prepare() != Some(origin) {
            return;
        }
        let session = self.remote.detaching.map(|(_, session)| session);
        self.act(WorkspaceIn::Split {
            towards: Direction::Right,
        });
        self.detach_finish();
        let Some(session) = session else {
            return;
        };
        let shown = self
            .remote
            .local
            .iter()
            .find(|(pane, own)| **pane != origin && own.resume == Some(session))
            .map(|(pane, _)| *pane);
        let Some(pane) = shown else {
            return;
        };
        if let Some(slot) = self.slots.get(&pane)
            && let Some(session) = slot.borrow_mut().session.as_mut()
        {
            session.inherit(history);
        }
        self.live_cards.shown.insert((origin, id), pane);
        self.live_cards.commands.insert((origin, id), command);
        self.card(
            origin,
            CardIn::Adopted {
                id,
                session: SurfaceId(session.0),
            },
        );
    }

    /// The pane showing the card whose session is `session`.
    fn card_pane(&self, origin: PaneId, session: SurfaceId) -> Option<PaneId> {
        let cards = self.live_cards.cards.get(&origin)?;
        let (id, _) = cards.iter().find(|(_, s)| s.session == session)?;
        self.live_cards.shown.get(&(origin, id)).copied()
    }

    /// Gives the card's pane the part of the split its size asks for, or the whole tab.
    fn card_resize(&mut self, origin: PaneId, session: SurfaceId, size: CardSize) {
        let Some(pane) = self.card_pane(origin, session) else {
            return;
        };
        let zoomed = self
            .book
            .ws
            .front()
            .and_then(|tab| self.book.ws.tabs.tab(tab))
            .is_some_and(|tab| tab.tree.zoomed().is_some());
        let full = size == CardSize::Full;
        if zoomed != full {
            self.act(WorkspaceIn::Focus(pane));
            self.act(WorkspaceIn::ToggleZoom);
        }
        // The share is the first pane's (the one the command ran in).
        let share = match size {
            CardSize::Compact => 750_000,
            CardSize::Normal | CardSize::Full => Share::HALF.millionths(),
            CardSize::Tall => 250_000,
        };
        self.act(WorkspaceIn::SetShare {
            before: origin,
            after: pane,
            share: Share::from(share),
        });
    }
}

impl HubRef {
    /// Whether the command running in `pane` can be moved to a live card.
    pub fn can_detach(&self, pane: PaneId) -> bool {
        self.0.borrow().can_detach(pane)
    }

    /// What the head of the live card shown in `pane` says.
    pub fn live_head(&self, pane: PaneId) -> Option<LiveHead> {
        self.0.borrow().live_head(pane)
    }
}

impl Desk {
    /// Moves the command running in `pane` to a live card.
    pub fn detach_block(&self, pane: PaneId) {
        self.settle(|hub| hub.detach_block(pane));
    }

    /// An icon of the head of the live card shown in `pane` was pressed.
    pub fn live_icon(&self, pane: PaneId, icon: LiveIcon) {
        self.settle(|hub| hub.live_icon(pane, icon));
    }
}
