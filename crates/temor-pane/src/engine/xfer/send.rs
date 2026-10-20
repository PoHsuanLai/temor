//! Answering a program that asks for a file (`ac=receive`): the shelf's file is offered with a
//! [`SendSession`] and streamed a few frames at a time, so a big file never piles up in the
//! pane's output queue.

use std::collections::VecDeque;
use std::fs::File;
use std::path::PathBuf;

use temor_transfer::Bytes;
use temor_transfer::Consent;
use temor_transfer::kitty::{
    ErrorCode, KittyId, Message, Offered, REFUSED_BY_PERSON, Reply, SendEffect, SendEnd, SendInput,
    SendSession, encode_message, read_chunk, serve,
};

use super::event::{OfferId, TransferEnd};
use super::state::{Reaction, Transfers, ended};

/// How many frames go out before the pane lets the program's input drain.
const FRAMES_AT_A_TIME: usize = 16;

/// One file on its way to the program.
#[derive(Debug)]
pub struct Sending {
    pub(super) offer: OfferId,
    session: Option<SendSession>,
    path: PathBuf,
    file: Option<File>,
    effects: VecDeque<SendEffect>,
    inputs: VecDeque<SendInput>,
}

impl Transfers {
    pub(super) fn answer_request(
        &mut self,
        offer: OfferId,
        request: Message,
        consent: Consent,
        shelf: &[Offered],
        out: &mut Reaction,
    ) {
        if consent == Consent::Refuse {
            out.refuse(request.id, ErrorCode::Eperm, REFUSED_BY_PERSON);
            out.events.push(ended(offer, TransferEnd::Refused));
            return;
        }
        match serve(&request, shelf) {
            Err(reply) => {
                out.reply(&reply);
                out.events.push(ended(offer, TransferEnd::Failed));
            }
            Ok(served) => {
                let sending = Sending {
                    offer,
                    session: Some(served.session),
                    path: served.path,
                    file: None,
                    effects: served.first.into(),
                    inputs: VecDeque::new(),
                };
                self.sends.insert(request.id.clone(), sending);
                self.advance_send(&request.id, out);
            }
        }
    }

    /// A status from the program about an offer this pane made.
    pub(super) fn on_status(&mut self, reply: Reply, out: &mut Reaction) {
        let id = reply.id.clone();
        if let Some(sending) = self.sends.get_mut(&id) {
            sending.inputs.push_back(SendInput::Reply(reply));
            self.advance_send(&id, out);
        }
    }

    /// Whether any file is still waiting to be streamed.
    pub fn sending(&self) -> bool {
        self.sends.values().any(Sending::has_work)
    }

    /// Streams the next frames of every file that has some. The caller calls this when the
    /// program's input has drained.
    pub fn pump_sends(&mut self) -> Reaction {
        let mut out = Reaction::default();
        let ids: Vec<KittyId> = self.sends.keys().cloned().collect();
        for id in ids {
            self.advance_send(&id, &mut out);
        }
        out
    }

    fn advance_send(&mut self, id: &KittyId, out: &mut Reaction) {
        let Some(sending) = self.sends.get_mut(id) else {
            return;
        };
        if let Some(end) = sending.advance(out) {
            let offer = sending.offer;
            self.sends.remove(id);
            out.events.push(ended(offer, how(&end)));
        }
    }
}

fn how(end: &SendEnd) -> TransferEnd {
    match end {
        SendEnd::Delivered => TransferEnd::Finished,
        SendEnd::Refused { .. } => TransferEnd::Refused,
        SendEnd::Cancelled => TransferEnd::Cancelled,
        _ => TransferEnd::Failed,
    }
}

impl Sending {
    fn has_work(&self) -> bool {
        !self.effects.is_empty() || !self.inputs.is_empty()
    }

    /// Carries out pending effects and inputs, at most [`FRAMES_AT_A_TIME`] frames, and returns
    /// how the transfer ended if it did.
    fn advance(&mut self, out: &mut Reaction) -> Option<SendEnd> {
        let mut frames = 0;
        while frames < FRAMES_AT_A_TIME {
            if let Some(effect) = self.effects.pop_front() {
                match effect {
                    SendEffect::Emit(message) => {
                        out.replies.push(encode_message(&message));
                        frames += 1;
                    }
                    SendEffect::Read { max, .. } => self.read(max),
                    SendEffect::Ended(end) => return Some(end),
                    _ => {}
                }
            } else if let Some(input) = self.inputs.pop_front() {
                let session = self.session.take()?;
                let (next, effects) = session.step(input);
                self.session = Some(next);
                self.effects.extend(effects);
            } else {
                break;
            }
        }
        None
    }

    /// Reads the next piece of the file; a file that cannot be read cancels the transfer.
    fn read(&mut self, max: Bytes) {
        if self.file.is_none() {
            self.file = File::open(&self.path).ok();
        }
        let chunk = self
            .file
            .as_mut()
            .and_then(|file| read_chunk(file, max, &self.path).ok());
        self.inputs.push_back(match chunk {
            Some((data, end)) => SendInput::Chunk { data, end },
            None => SendInput::Aborted,
        });
    }
}
