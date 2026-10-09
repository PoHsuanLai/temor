//! Small builders shared by the step arms: replies, failures and the ways a session ends.

use std::collections::BTreeMap;

use super::model::{Effect, EndWhy, Phase, ReceiveSession, WriteOp};
use crate::kitty::model::{ErrorCode, FileId, KittyId, Reply, ReplyStatus};

/// Answers with a failure for one entry and keeps the session going.
pub(super) fn refuse(
    session: ReceiveSession,
    id: &KittyId,
    file: Option<FileId>,
    code: ErrorCode,
    message: &str,
) -> (ReceiveSession, Vec<Effect>) {
    refuse_with(session, id, file, failed(code, message))
}

pub(super) fn refuse_with(
    session: ReceiveSession,
    id: &KittyId,
    file: Option<FileId>,
    status: ReplyStatus,
) -> (ReceiveSession, Vec<Effect>) {
    (session, vec![reply(id, file, status)])
}

/// Ends the session: partly written files are discarded, then the reply and the end are sent.
pub(super) fn close(
    session: ReceiveSession,
    id: &KittyId,
    status: ReplyStatus,
    why: EndWhy,
) -> (ReceiveSession, Vec<Effect>) {
    let mut effects = discards(&session);
    effects.push(reply(id, None, status));
    effects.push(Effect::Ended(why));
    (ended(session), effects)
}

pub(super) fn discards(session: &ReceiveSession) -> Vec<Effect> {
    session
        .open
        .values()
        .map(|file| Effect::Write(WriteOp::Discard(file.path.clone())))
        .collect()
}

pub(super) fn ended(session: ReceiveSession) -> ReceiveSession {
    ReceiveSession {
        phase: Phase::Done,
        open: BTreeMap::new(),
        ..session
    }
}

pub(super) fn with_phase(session: ReceiveSession, phase: Phase) -> ReceiveSession {
    ReceiveSession { phase, ..session }
}

pub(super) fn failed(code: ErrorCode, message: &str) -> ReplyStatus {
    ReplyStatus::Failed {
        code,
        message: message.to_owned(),
    }
}

pub(super) fn reply(id: &KittyId, file: Option<FileId>, status: ReplyStatus) -> Effect {
    Effect::Reply(Reply {
        id: id.clone(),
        file,
        status,
    })
}
