//! The pane's transfer state and where each sequence goes. Pure but for the files the staging
//! folder holds: every method returns a [`Reaction`] (bytes for the program, events for the
//! window) and the I/O thread carries it out.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use temor_transfer::Consent;
use temor_transfer::kitty::{
    Action, ErrorCode, KittyId, Message, Offered, Reply, ReplyStatus, encode_reply, parse_message,
    parse_reply, readable_header,
};
use temor_transfer::osc1337::{FileOffer, Osc1337Reader};
use temor_vt::UnknownKind;

use super::event::{OfferId, TransferEvent, TransferSetup};
use super::receive::Receive;
use super::send::Sending;

/// The most offers and requests a pane holds at once. A program that sends more is refused,
/// so a flood of offers cannot fill the window with questions.
pub(super) const MOST_AT_ONCE: usize = 8;

const KITTY_HEAD: &[u8] = b"5113;";

/// What one step asks of the I/O thread.
#[derive(Debug, Default)]
pub struct Reaction {
    /// Encoded sequences for the program, in order.
    pub replies: Vec<Vec<u8>>,
    pub events: Vec<TransferEvent>,
}

impl Reaction {
    pub(super) fn reply(&mut self, reply: &Reply) {
        self.replies.push(encode_reply(reply));
    }

    pub(super) fn refuse(&mut self, id: KittyId, code: ErrorCode, message: &str) {
        self.reply(&Reply {
            id,
            file: None,
            status: ReplyStatus::Failed {
                code,
                message: message.to_owned(),
            },
        });
    }
}

/// Everything a pane holds about transfers. Lives on the I/O thread.
#[derive(Debug)]
pub struct Transfers {
    pub(super) setup: TransferSetup,
    pub(super) reader: Osc1337Reader,
    next_offer: u64,
    next_dir: u64,
    pub(super) inline: BTreeMap<OfferId, FileOffer>,
    pub(super) receives: BTreeMap<KittyId, Receive>,
    pub(super) wants: BTreeMap<OfferId, Message>,
    pub(super) sends: BTreeMap<KittyId, Sending>,
}

impl Transfers {
    pub fn new(setup: TransferSetup) -> Self {
        let reader = Osc1337Reader::new(setup.limits);
        Self {
            setup,
            reader,
            next_offer: 1,
            next_dir: 1,
            inline: BTreeMap::new(),
            receives: BTreeMap::new(),
            wants: BTreeMap::new(),
            sends: BTreeMap::new(),
        }
    }

    pub(super) fn fresh_offer(&mut self) -> OfferId {
        let id = OfferId(self.next_offer);
        self.next_offer += 1;
        id
    }

    pub(super) fn busy(&self) -> bool {
        self.inline.len() + self.receives.len() + self.wants.len() >= MOST_AT_ONCE
    }

    /// A fresh, empty folder inside the pane's staging folder.
    pub(super) fn new_dir(&mut self) -> io::Result<PathBuf> {
        fs::create_dir_all(&self.setup.staging)?;
        loop {
            let path = self.setup.staging.join(format!("offer-{}", self.next_dir));
            self.next_dir += 1;
            match fs::create_dir(&path) {
                Ok(()) => return Ok(path),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err),
            }
        }
    }

    /// A sequence libghostty did not take (or the scanner took out first). Anything that is not a
    /// transfer is ignored.
    pub fn on_unknown(&mut self, kind: UnknownKind, payload: &[u8], truncation: bool) -> Reaction {
        let mut out = Reaction::default();
        match kind {
            UnknownKind::Osc1337File => self.inline_file(payload, truncation, &mut out),
            UnknownKind::Osc if is_multipart(payload) => {
                self.inline_file(payload, truncation, &mut out);
            }
            UnknownKind::Osc if payload.starts_with(KITTY_HEAD) => {
                self.kitty_frame(payload, truncation, &mut out);
            }
            _ => {}
        }
        out
    }

    fn kitty_frame(&mut self, payload: &[u8], truncation: bool, out: &mut Reaction) {
        let limits = self.setup.limits;
        let whole = !truncation;
        if whole && let Ok(reply) = parse_reply(payload, limits) {
            self.on_status(reply, out);
            return;
        }
        let message = match whole.then(|| parse_message(payload, limits)) {
            Some(Ok(message)) => message,
            _ => return self.unreadable(payload, out),
        };
        match message.action {
            Action::Receive => self.want(message, out),
            Action::Send if !self.receives.contains_key(&message.id) => {
                self.begin_receive(message, out);
            }
            _ => self.receive_frame(message, out),
        }
    }

    /// A frame that did not parse. Only a transfer this pane knows is told so; a stranger's is
    /// answered once, so its sender does not wait.
    fn unreadable(&mut self, payload: &[u8], out: &mut Reaction) {
        let Some(id) = readable_header(payload, self.setup.limits) else {
            return;
        };
        if self.receives.contains_key(&id) {
            self.receive_malformed(&id, out);
        } else if !self.sends.contains_key(&id) {
            out.refuse(id, ErrorCode::Einval, "the frame could not be read");
        }
    }

    fn want(&mut self, message: Message, out: &mut Reaction) {
        if self.busy() {
            out.refuse(message.id, ErrorCode::Eperm, "too many transfers at once");
            return;
        }
        let offer = self.fresh_offer();
        out.events.push(TransferEvent::Wanted {
            offer,
            name: message.name.clone(),
        });
        self.wants.insert(offer, message);
    }

    /// The window's answer to an offer.
    pub fn decide(&mut self, offer: OfferId, consent: Consent) -> Reaction {
        let mut out = Reaction::default();
        if self.inline.contains_key(&offer) {
            self.decide_inline(offer, consent, &mut out);
        } else if let Some(id) = self.receive_of(offer) {
            self.decide_receive(&id, consent, &mut out);
        }
        out
    }

    /// The window's answer to a request, with the files the shelf can hand out.
    pub fn serve(&mut self, offer: OfferId, consent: Consent, shelf: &[Offered]) -> Reaction {
        let mut out = Reaction::default();
        if let Some(request) = self.wants.remove(&offer) {
            self.answer_request(offer, request, consent, shelf, &mut out);
        }
        out
    }

    /// The program ended or the pane closed: nothing more will come, and what is half done is
    /// thrown away.
    pub fn abort_all(&mut self) -> Reaction {
        let mut out = Reaction::default();
        for offer in std::mem::take(&mut self.inline).into_keys() {
            out.events
                .push(ended(offer, super::event::TransferEnd::Aborted));
        }
        for offer in std::mem::take(&mut self.wants).into_keys() {
            out.events
                .push(ended(offer, super::event::TransferEnd::Aborted));
        }
        for sending in std::mem::take(&mut self.sends).into_values() {
            out.events
                .push(ended(sending.offer, super::event::TransferEnd::Aborted));
        }
        let ids: Vec<KittyId> = self.receives.keys().cloned().collect();
        for id in ids {
            self.receive_aborted(&id, &mut out);
        }
        out
    }
}

pub(super) fn ended(offer: OfferId, how: super::event::TransferEnd) -> TransferEvent {
    TransferEvent::Ended { offer, how }
}

/// Whether an OSC 1337 payload is a later piece of a multipart file.
fn is_multipart(payload: &[u8]) -> bool {
    payload.starts_with(b"1337;MultipartFile=")
        || payload.starts_with(b"1337;FilePart=")
        || payload == b"1337;FileEnd"
}
