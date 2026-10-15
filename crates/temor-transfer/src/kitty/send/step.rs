//! `SendSession::step`: offer, then stream each file in order, then finish.

use super::model::{CHUNK, ChunkEnd, Phase, SendEffect, SendEnd, SendFile, SendInput, SendSession};
use crate::kitty::model::{
    Action, FileId, FileType, FirstAnswer, KittyId, Message, Reply, ReplyStatus, Transmission,
};
use crate::units::Bytes;

impl SendSession {
    /// Starts a transfer of `files` under `id`. The first effect is the offer.
    pub fn new(id: KittyId, files: Vec<SendFile>) -> (Self, Vec<SendEffect>) {
        let offer = Message {
            transmission: Some(Transmission::Simple),
            first_answer: FirstAnswer::StartedAtOnce,
            ..Message::new(Action::Send, id.clone())
        };
        let session = Self {
            id,
            files,
            phase: Phase::Offered,
        };
        (session, vec![SendEffect::Emit(offer)])
    }

    /// The id every frame of this transfer carries.
    pub fn id(&self) -> &KittyId {
        &self.id
    }

    /// The step: `(session, input) -> (session, effects)`. Pure.
    pub fn step(self, input: SendInput) -> (Self, Vec<SendEffect>) {
        match (self.phase.clone(), input) {
            (Phase::Done, _) => (self, Vec::new()),
            (_, SendInput::Aborted) => self.end(SendEnd::Cancelled, true),
            (phase, SendInput::Reply(reply)) if reply.id == self.id => self.on_reply(&phase, reply),
            (Phase::Streaming(index), SendInput::Chunk { data, end }) => {
                self.on_chunk(index, data, end)
            }
            _ => (self, Vec::new()),
        }
    }

    fn on_reply(self, phase: &Phase, reply: Reply) -> (Self, Vec<SendEffect>) {
        match (phase, reply.status) {
            (Phase::Offered, ReplyStatus::Ok) if reply.file.is_none() => self.begin(0),
            (Phase::Offered, ReplyStatus::Failed { message, .. }) => {
                self.end(SendEnd::Refused { message }, false)
            }
            (Phase::Offered, ReplyStatus::Cancelled) => self.end(
                SendEnd::Refused {
                    message: "the other side said no".to_owned(),
                },
                false,
            ),
            (Phase::Finishing, ReplyStatus::Ok) if reply.file.is_none() => {
                self.end(SendEnd::Delivered, false)
            }
            (_, ReplyStatus::Failed { message, .. }) => self.end(SendEnd::Failed { message }, true),
            (_, ReplyStatus::Cancelled) => self.end(SendEnd::Cancelled, false),
            _ => (self, Vec::new()),
        }
    }

    /// Announces file `index` and asks for its first chunk, or finishes when there are no more.
    fn begin(self, index: usize) -> (Self, Vec<SendEffect>) {
        let Some(file) = self.files.get(index) else {
            let finish = Message::new(Action::Finish, self.id.clone());
            let next = Self {
                phase: Phase::Finishing,
                ..self
            };
            return (next, vec![SendEffect::Emit(finish)]);
        };
        let meta = Message {
            file: fid(index),
            name: Some(file.name.as_str().to_owned()),
            file_type: Some(FileType::Regular),
            size: Some(file.size),
            mtime: file.modified,
            ..Message::new(Action::File, self.id.clone())
        };
        let read = SendEffect::Read {
            file: index,
            max: Bytes::new(CHUNK),
        };
        let next = Self {
            phase: Phase::Streaming(index),
            ..self
        };
        (next, vec![SendEffect::Emit(meta), read])
    }

    fn on_chunk(self, index: usize, data: Vec<u8>, end: ChunkEnd) -> (Self, Vec<SendEffect>) {
        let action = match end {
            ChunkEnd::More => Action::Data,
            ChunkEnd::Last => Action::EndData,
        };
        let frame = SendEffect::Emit(Message {
            file: fid(index),
            data,
            ..Message::new(action, self.id.clone())
        });
        match end {
            ChunkEnd::More => {
                let read = SendEffect::Read {
                    file: index,
                    max: Bytes::new(CHUNK),
                };
                (self, vec![frame, read])
            }
            ChunkEnd::Last => {
                let (next, mut effects) = self.begin(index + 1);
                effects.insert(0, frame);
                (next, effects)
            }
        }
    }

    /// Ends the transfer. `tell` also sends a cancel, for an end the receiver has not caused.
    fn end(self, how: SendEnd, tell: bool) -> (Self, Vec<SendEffect>) {
        let mut effects = Vec::new();
        if tell {
            effects.push(SendEffect::Emit(Message::new(
                Action::Cancel,
                self.id.clone(),
            )));
        }
        effects.push(SendEffect::Ended(how));
        let next = Self {
            phase: Phase::Done,
            ..self
        };
        (next, effects)
    }
}

/// File number `index` as the transfer's file id: `1`, `2`, ...
fn fid(index: usize) -> Option<FileId> {
    FileId::parse(&(index + 1).to_string()).ok()
}
