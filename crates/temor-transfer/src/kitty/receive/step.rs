//! The transitions of the receive machine. Each arm returns the next session and its effects.

use std::collections::BTreeMap;

use super::model::{
    Consent, Effect, EndWhy, Input, OpenFile, Phase, ReceiveSession, SavedFile, WriteOp,
};
use crate::kitty::model::{
    Action, Compression, ErrorCode, FileId, FileType, KittyId, Message, Reply, ReplyStatus,
};
use crate::name::RelPath;
use crate::units::Bytes;

pub(crate) fn step(session: ReceiveSession, input: Input) -> (ReceiveSession, Vec<Effect>) {
    match (session.phase.clone(), input) {
        (Phase::Idle, Input::Frame(message)) if message.action == Action::Send => {
            let ask = Effect::AskConsent(session.origin.clone());
            (with_phase(session, Phase::Asking(message.id)), vec![ask])
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
        (_, _) => (session, Vec::new()),
    }
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

/// Whether a data frame carries the last chunk of its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chunk {
    More,
    Last,
}

fn on_file(
    session: ReceiveSession,
    id: &KittyId,
    message: Message,
) -> (ReceiveSession, Vec<Effect>) {
    let Some(file_id) = message.file.clone() else {
        return refuse(session, id, None, ErrorCode::Einval, "the file has no id");
    };
    if session.dirs.contains_key(&file_id) || session.open.contains_key(&file_id) {
        return refuse(
            session,
            id,
            Some(file_id),
            ErrorCode::Einval,
            "the file id is reused",
        );
    }
    if message.compression == Some(Compression::Zlib) {
        return refuse(
            session,
            id,
            Some(file_id),
            ErrorCode::Einval,
            "compressed data is not supported",
        );
    }
    let path = match entry_path(&session, &message) {
        Ok(path) => path,
        Err((code, why)) => return refuse(session, id, Some(file_id), code, why),
    };
    match message.file_type.unwrap_or(FileType::Regular) {
        FileType::Directory => {
            let mut next = session;
            next.dirs.insert(file_id.clone(), path.clone());
            let ok = reply(id, Some(file_id), ReplyStatus::Ok);
            (next, vec![Effect::Write(WriteOp::MakeDir(path)), ok])
        }
        FileType::Regular => open_regular(session, id, file_id, path, &message),
        FileType::Symlink | FileType::Link => refuse(
            session,
            id,
            Some(file_id),
            ErrorCode::Eperm,
            "links are not received",
        ),
    }
}

fn entry_path(
    session: &ReceiveSession,
    message: &Message,
) -> Result<RelPath, (ErrorCode, &'static str)> {
    let name = message
        .name
        .as_deref()
        .ok_or((ErrorCode::Einval, "the file has no name"))?;
    let relative =
        RelPath::parse(name).map_err(|_| (ErrorCode::Eperm, "the path is not allowed"))?;
    let base = match &message.parent {
        None => RelPath::root(),
        Some(parent) => session
            .dirs
            .get(parent)
            .cloned()
            .ok_or((ErrorCode::Enoent, "the folder is not known"))?,
    };
    Ok(base.join(&relative))
}

fn open_regular(
    session: ReceiveSession,
    id: &KittyId,
    file_id: FileId,
    path: RelPath,
    message: &Message,
) -> (ReceiveSession, Vec<Effect>) {
    if let Some(size) = message.size
        && session.received + size.get() > session.limits.max_bytes().get()
    {
        let too_big = failed(ErrorCode::Efbig, "the transfer is larger than the limit");
        return refuse_with(session, id, Some(file_id), too_big);
    }
    let mut next = session;
    next.open.insert(
        file_id.clone(),
        OpenFile {
            path: path.clone(),
            expected: message.size,
            written: 0,
            modified: message.mtime,
        },
    );
    let started = reply(id, Some(file_id), ReplyStatus::Started);
    (
        next,
        vec![Effect::Write(WriteOp::CreateFile(path)), started],
    )
}

fn on_data(
    session: ReceiveSession,
    id: &KittyId,
    message: Message,
    chunk: Chunk,
) -> (ReceiveSession, Vec<Effect>) {
    let Some(file_id) = message.file.clone() else {
        return refuse(
            session,
            id,
            None,
            ErrorCode::Einval,
            "the data has no file id",
        );
    };
    let Some(open) = session.open.get(&file_id).cloned() else {
        return refuse(
            session,
            id,
            Some(file_id),
            ErrorCode::Enoent,
            "the file is not open",
        );
    };
    let len = message.data.len() as u64;
    let total = session.received + len;
    if total > session.limits.max_bytes().get() {
        let too_big = failed(ErrorCode::Efbig, "the transfer is larger than the limit");
        return close(session, id, too_big, EndWhy::Failed);
    }
    let written = open.written + len;
    if open
        .expected
        .is_some_and(|expected| written > expected.get())
    {
        let extra = failed(ErrorCode::Eio, "more data arrived than announced");
        return close(session, id, extra, EndWhy::Failed);
    }
    let mut next = session;
    next.received = total;
    let mut effects = Vec::new();
    if !message.data.is_empty() {
        effects.push(Effect::Write(WriteOp::Append {
            path: open.path.clone(),
            data: message.data,
        }));
    }
    if chunk == Chunk::More {
        if let Some(file) = next.open.get_mut(&file_id) {
            file.written = written;
        }
        return (next, effects);
    }
    if open
        .expected
        .is_some_and(|expected| expected.get() != written)
    {
        let short = failed(ErrorCode::Eio, "the size does not match the announced size");
        return close(next, id, short, EndWhy::Failed);
    }
    next.open.remove(&file_id);
    effects.push(Effect::Write(WriteOp::Finish {
        path: open.path.clone(),
        modified: open.modified,
    }));
    effects.push(reply(id, Some(file_id), ReplyStatus::Ok));
    effects.push(Effect::Saved(SavedFile {
        path: open.path,
        size: Bytes::new(written),
    }));
    (next, effects)
}

/// Answers with a failure for one entry and keeps the session going.
fn refuse(
    session: ReceiveSession,
    id: &KittyId,
    file: Option<FileId>,
    code: ErrorCode,
    message: &str,
) -> (ReceiveSession, Vec<Effect>) {
    refuse_with(session, id, file, failed(code, message))
}

fn refuse_with(
    session: ReceiveSession,
    id: &KittyId,
    file: Option<FileId>,
    status: ReplyStatus,
) -> (ReceiveSession, Vec<Effect>) {
    (session, vec![reply(id, file, status)])
}

/// Ends the session: partly written files are discarded, then the reply and the end are sent.
fn close(
    session: ReceiveSession,
    id: &KittyId,
    status: ReplyStatus,
    why: EndWhy,
) -> (ReceiveSession, Vec<Effect>) {
    let mut effects: Vec<Effect> = session
        .open
        .values()
        .map(|file| Effect::Write(WriteOp::Discard(file.path.clone())))
        .collect();
    effects.push(reply(id, None, status));
    effects.push(Effect::Ended(why));
    let next = ReceiveSession {
        phase: Phase::Done,
        open: BTreeMap::new(),
        ..session
    };
    (next, effects)
}

fn with_phase(session: ReceiveSession, phase: Phase) -> ReceiveSession {
    ReceiveSession { phase, ..session }
}

fn failed(code: ErrorCode, message: &str) -> ReplyStatus {
    ReplyStatus::Failed {
        code,
        message: message.to_owned(),
    }
}

fn reply(id: &KittyId, file: Option<FileId>, status: ReplyStatus) -> Effect {
    Effect::Reply(Reply {
        id: id.clone(),
        file,
        status,
    })
}
