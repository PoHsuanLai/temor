//! Helpers for the receive-machine tests: frames, effects and small builders.

use temor_transfer::kitty::{
    Action, Effect, FileId, FileType, Input, KittyId, Message, ReceiveSession, ReplyStatus, WriteOp,
};
use temor_transfer::{Bytes, Consent, HostName, Origin};

use crate::support::limits;

pub fn origin() -> Origin {
    Origin::Host {
        host: HostName::new("prod-1"),
    }
}

pub fn session(max: u64) -> ReceiveSession {
    ReceiveSession::new(origin(), limits(max))
}

pub fn msg(action: Action) -> Message {
    Message::new(action, KittyId::parse("tx1").expect("id"))
}

pub fn fid(raw: &str) -> FileId {
    FileId::parse(raw).expect("fid")
}

pub fn send(s: ReceiveSession) -> (ReceiveSession, Vec<Effect>) {
    s.step(Input::Frame(msg(Action::Send)))
}

pub fn allowed(max: u64) -> ReceiveSession {
    let (s, _) = send(session(max));
    let (s, _) = s.step(Input::Decided(Consent::Allow));
    s
}

pub fn file(name: &str, id: &str, kind: FileType, size: Option<u64>) -> Input {
    Input::Frame(Message {
        file: Some(fid(id)),
        name: Some(name.to_owned()),
        file_type: Some(kind),
        size: size.map(Bytes::new),
        ..msg(Action::File)
    })
}

pub fn writes(effects: &[Effect]) -> Vec<WriteOp> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Write(op) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

pub fn replies(effects: &[Effect]) -> Vec<ReplyStatus> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Reply(reply) => Some(reply.status.clone()),
            _ => None,
        })
        .collect()
}
