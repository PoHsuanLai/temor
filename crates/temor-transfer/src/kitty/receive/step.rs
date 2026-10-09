//! The transitions of the receive machine. Each arm returns the next session and its effects.

use super::files::{Chunk, on_data, on_file};
use super::helpers::{close, discards, ended, failed, reply, with_phase};
use super::model::{Effect, EndWhy, Input, Phase, ReceiveSession, WriteOp};
use crate::consent::Consent;
use crate::kitty::model::{Action, ErrorCode, FileId, KittyId, Message, ReplyStatus, Transmission};

pub(crate) fn step(session: ReceiveSession, input: Input) -> (ReceiveSession, Vec<Effect>) {
    match (session.phase.clone(), input) {
        (Phase::Done, _) => (session, Vec::new()),
        (Phase::Idle, Input::Frame(message)) if message.action == Action::Send => {
            offer(session, message)
        }
        (Phase::Asking(id), Input::Decided(Consent::Allow)) => {
            let ok = reply(&id, None, ReplyStatus::Ok);
            (with_phase(session, Phase::Receiving(id)), vec![ok])
        }
        (Phase::Asking(id), Input::Decided(Consent::Refuse)) => {
            let status = failed(ErrorCode::Eperm, "User refused the transfer");
            close(session, &id, status, EndWhy::Refused)
        }
        (Phase::Receiving(id), Input::Frame(message)) => on_frame(session, &id, message),
        (Phase::Receiving(id), Input::WriteFailed { fid, error }) => {
            on_write_failed(session, &id, fid, error)
        }
        (_, Input::Aborted) => abort(session),
        (_, _) => (session, Vec::new()),
    }
}

/// An offer. Only the simple transmission type is implemented; anything else is refused before
/// the person is asked.
fn offer(session: ReceiveSession, message: Message) -> (ReceiveSession, Vec<Effect>) {
    if message
        .transmission
        .is_some_and(|tt| tt != Transmission::Simple)
    {
        let status = failed(
            ErrorCode::Eperm,
            "only the simple transmission type is supported",
        );
        return close(session, &message.id, status, EndWhy::Failed);
    }
    let ask = Effect::AskConsent(session.origin.clone());
    (with_phase(session, Phase::Asking(message.id)), vec![ask])
}

fn on_frame(
    session: ReceiveSession,
    id: &KittyId,
    message: Message,
) -> (ReceiveSession, Vec<Effect>) {
    match message.action {
        Action::File => on_file(session, id, message),
        Action::Data => on_data(session, id, message, Chunk::More),
        Action::EndData => on_data(session, id, message, Chunk::Last),
        Action::Finish => close(session, id, ReplyStatus::Ok, EndWhy::Finished),
        Action::Cancel => close(session, id, ReplyStatus::Cancelled, EndWhy::Cancelled),
        Action::Send | Action::Status | Action::Unknown => {
            let refusal = failed(ErrorCode::Einval, "unexpected action");
            (session, vec![reply(id, None, refusal)])
        }
    }
}

/// A write failed at the edge. A failure for an open file cancels that file and the transfer
/// goes on. Any other failure ends the transfer.
fn on_write_failed(
    session: ReceiveSession,
    id: &KittyId,
    fid: Option<FileId>,
    error: ErrorCode,
) -> (ReceiveSession, Vec<Effect>) {
    let status = failed(error, "the file could not be written");
    let Some(file_id) = fid.filter(|file_id| session.open.contains_key(file_id)) else {
        let status = failed(error, "the transfer could not be written");
        return close(session, id, status, EndWhy::Failed);
    };
    let mut next = session;
    let Some(open) = next.open.remove(&file_id) else {
        return (next, Vec::new());
    };
    let effects = vec![
        Effect::Write(WriteOp::Discard(open.path)),
        reply(id, Some(file_id), status),
    ];
    (next, effects)
}

/// The stream was dropped: partly written files are discarded and nothing is replied, since
/// there is no one to reply to.
fn abort(session: ReceiveSession) -> (ReceiveSession, Vec<Effect>) {
    let mut effects = discards(&session);
    effects.push(Effect::Ended(EndWhy::Aborted));
    (ended(session), effects)
}
