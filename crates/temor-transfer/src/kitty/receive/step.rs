//! The transitions of the receive machine. Each arm returns the next session and its effects.

use super::files::{Chunk, on_data, on_file};
use super::helpers::{close, discards, ended, failed, reply, with_phase};
use super::model::{Effect, EndWhy, Input, Phase, ReceiveSession, SavedFile, WriteOp};
use crate::consent::Consent;
use crate::kitty::model::{
    Action, ErrorCode, FileId, KittyId, Message, ReplyStatus, Transmission, WriteKind,
};
use crate::units::Bytes;

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
        (Phase::Receiving(id), Input::WriteFailed { fid, op, error }) => {
            on_write_failed(session, &id, fid, op, error)
        }
        (Phase::Receiving(id), Input::WriteDone { fid }) => on_write_done(session, &id, fid),
        (_, Input::Malformed { id }) => malformed(session, &id),
        (_, Input::Aborted) => abort(session),
        (_, _) => (session, Vec::new()),
    }
}

/// An offer. Only the simple transmission type is implemented; anything else is refused before
/// the person is asked.
fn offer(session: ReceiveSession, message: Message) -> (ReceiveSession, Vec<Effect>) {
    if message
        .transmission
        .as_ref()
        .is_some_and(|tt| *tt != Transmission::Simple)
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
/// A write failed at the edge. The failed operation decides what is on disk. A failed `Create`
/// or `Append` cancels that file (only `Append` leaves something to discard, and `Create` never
/// got past `EEXIST` or a similar refusal, so nothing of ours is at that path). A failed `Finish`
/// cancels the file that was complete. Any other failure ends the transfer.
fn on_write_failed(
    session: ReceiveSession,
    id: &KittyId,
    fid: Option<FileId>,
    op: WriteKind,
    error: ErrorCode,
) -> (ReceiveSession, Vec<Effect>) {
    let status = failed(error, "the file could not be written");
    let whole = failed(error, "the transfer could not be written");
    let Some(file_id) = fid else {
        return close(session, id, whole, EndWhy::Failed);
    };
    let mut next = session;
    let (found, discard) = match op {
        WriteKind::Create => (next.open.remove(&file_id), false),
        WriteKind::Append => (next.open.remove(&file_id), true),
        WriteKind::Finish => (next.finishing.remove(&file_id), true),
        WriteKind::MakeDir | WriteKind::Discard => (None, false),
    };
    let Some(file) = found else {
        return close(next, id, whole, EndWhy::Failed);
    };
    let mut effects = Vec::new();
    if discard {
        effects.push(Effect::Write(WriteOp::Discard(file.path)));
    }
    effects.push(reply(id, Some(file_id), status));
    (next, effects)
}

/// The `Finish` of a complete file succeeded: now it is saved.
fn on_write_done(
    session: ReceiveSession,
    id: &KittyId,
    fid: FileId,
) -> (ReceiveSession, Vec<Effect>) {
    let mut next = session;
    let Some(file) = next.finishing.remove(&fid) else {
        return (next, Vec::new());
    };
    let effects = vec![
        reply(id, Some(fid), ReplyStatus::Ok),
        Effect::Saved(SavedFile {
            path: file.path,
            size: Bytes::new(file.written),
        }),
    ];
    (next, effects)
}

/// A frame that did not parse but whose id was readable. It is answered only for the transfer it
/// belongs to. If files are open or finishing, the frame may have been a chunk, so the transfer
/// is closed rather than saved with a gap: the open files are discarded. A frame whose id is not
/// this transfer's is ignored. After the session is done, the Done arm ignores every input.
fn malformed(session: ReceiveSession, id: &KittyId) -> (ReceiveSession, Vec<Effect>) {
    let status = failed(ErrorCode::Einval, "the frame could not be read");
    let owner = match &session.phase {
        Phase::Asking(own) | Phase::Receiving(own) => Some(own),
        Phase::Idle | Phase::Done => None,
    };
    if owner != Some(id) {
        return (session, Vec::new());
    }
    if !session.open.is_empty() || !session.finishing.is_empty() {
        return close(session, id, status, EndWhy::Failed);
    }
    (session, vec![reply(id, None, status)])
}

/// The stream was dropped: partly written files are discarded and nothing is replied, since
/// there is no one to reply to.
fn abort(session: ReceiveSession) -> (ReceiveSession, Vec<Effect>) {
    let mut effects = discards(&session);
    effects.push(Effect::Ended(EndWhy::Aborted));
    (ended(session), effects)
}
