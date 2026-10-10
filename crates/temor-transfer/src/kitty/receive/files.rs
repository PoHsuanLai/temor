//! File and data frames: a new file is checked and opened, its data is appended, and its end is
//! checked against the announced size.

use super::helpers::{close, failed, refuse, refuse_with, reply};
use super::links::{Last, on_link_data, open_link};
use super::model::{Effect, EndWhy, LinkPolicy, OpenFile, ReceiveSession, WriteOp};
use crate::kitty::model::{
    Compression, ErrorCode, FileId, FileType, KittyId, Message, ReplyStatus,
};
use crate::name::RelPath;

/// Whether a data frame carries the last chunk of its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Chunk {
    More,
    Last,
}

pub(super) fn on_file(
    session: ReceiveSession,
    id: &KittyId,
    message: Message,
) -> (ReceiveSession, Vec<Effect>) {
    let Some(file_id) = message.file.clone() else {
        return refuse(session, id, None, ErrorCode::Einval, "the file has no id");
    };
    if session.dirs.contains_key(&file_id)
        || session.open.contains_key(&file_id)
        || session.finishing.contains_key(&file_id)
    {
        return refuse(
            session,
            id,
            Some(file_id),
            ErrorCode::Einval,
            "the file id is reused",
        );
    }
    if session.entries >= session.limits.max_entries().get() {
        let status = failed(
            ErrorCode::Efbig,
            "the transfer has more files and folders than the limit",
        );
        return close(session, id, status, EndWhy::Failed);
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
    // TODO(platform:macos) / TODO(platform:windows): those file systems are case-insensitive or
    // normalise names, so `Report.txt` and `report.txt` collide there. The claim must then be
    // keyed by a case-folded, normalised form of the path; until then, only exact matches are seen.
    if session.claimed.contains(&path) {
        return refuse(
            session,
            id,
            Some(file_id),
            ErrorCode::Eexist,
            "the name is already used in this transfer",
        );
    }
    match message.file_type.unwrap_or(FileType::Regular) {
        FileType::Directory => {
            let mut next = session;
            next.dirs.insert(file_id.clone(), path.clone());
            next.claimed.insert(path.clone());
            next.entries += 1;
            let ok = reply(id, Some(file_id), ReplyStatus::Ok);
            let mut effects = vec![Effect::Write(WriteOp::MakeDir(path.clone()))];
            if let Some(mode) = message.permissions {
                effects.push(Effect::Write(WriteOp::SetDirMode { path, mode }));
            }
            effects.push(ok);
            (next, effects)
        }
        FileType::Regular => open_regular(session, id, file_id, path, &message),
        FileType::Symlink if session.links == LinkPolicy::Make => {
            open_link(session, id, file_id, path)
        }
        FileType::Symlink | FileType::Link => refuse(
            session,
            id,
            Some(file_id),
            ErrorCode::Eperm,
            "links are not received",
        ),
    }
}

pub(super) fn entry_path(
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

pub(super) fn open_regular(
    session: ReceiveSession,
    id: &KittyId,
    file_id: FileId,
    path: RelPath,
    message: &Message,
) -> (ReceiveSession, Vec<Effect>) {
    if session.open.len() as u64 >= session.limits.max_open().get() {
        let status = failed(
            ErrorCode::Efbig,
            "more files would be open at once than the limit",
        );
        return close(session, id, status, EndWhy::Failed);
    }
    if let Some(size) = message.size
        && session.received + size.get() > session.limits.max_bytes().get()
    {
        let too_big = failed(ErrorCode::Efbig, "the transfer is larger than the limit");
        return refuse_with(session, id, Some(file_id), too_big);
    }
    let mut next = session;
    next.entries += 1;
    next.claimed.insert(path.clone());
    next.open.insert(
        file_id.clone(),
        OpenFile {
            path: path.clone(),
            expected: message.size,
            written: 0,
            modified: message.mtime,
            mode: message.permissions,
        },
    );
    let started = reply(id, Some(file_id), ReplyStatus::Started);
    (
        next,
        vec![Effect::Write(WriteOp::CreateFile(path)), started],
    )
}

pub(super) fn on_data(
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
    if session.open_links.contains_key(&file_id) {
        let last = match chunk {
            Chunk::More => Last::No,
            Chunk::Last => Last::Yes,
        };
        return on_link_data(session, id, file_id, message, last);
    }
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
    // The file is saved only when its Finish succeeds; the edge feeds that back as `WriteDone`.
    next.open.remove(&file_id);
    effects.push(Effect::Write(WriteOp::Finish {
        path: open.path.clone(),
        modified: open.modified,
        mode: open.mode,
    }));
    next.finishing.insert(file_id, OpenFile { written, ..open });
    (next, effects)
}
