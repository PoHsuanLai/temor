//! Asks the other side for a file and saves what it offers, with a `ReceiveSession`.

use std::collections::VecDeque;

use super::{Link, Waits};
use crate::Root;
use crate::consent::Consent;
use crate::error::TransferError;
use crate::kitty::frames::FrameScanner;
use crate::kitty::io::{apply_write, wire_code};
use crate::kitty::model::{FileId, KittyId, Message, ReplyStatus, WriteKind};
use crate::kitty::receive::{
    Effect, EndWhy, Input, LinkPolicy, ReceiveSession, SavedFile, WriteOp,
};
use crate::kitty::wire::{
    encode_message, encode_reply, parse_message, parse_reply, readable_header,
};
use crate::origin::{Origin, PaneKey, TabKey};
use crate::units::Limits;

/// Writes `request` to the link, then receives what the other side offers under the request's
/// id into `root`. The caller asked, so the offer is accepted without asking anyone; frames
/// with another id are ignored. Returns the files that were saved.
pub fn drive_receive(
    link: &mut impl Link,
    root: &Root,
    request: &Message,
    limits: Limits,
    waits: Waits,
) -> Result<Vec<SavedFile>, TransferError> {
    link.write(&encode_message(request))
        .map_err(TransferError::Link)?;
    // The origin is only a label here; nobody is asked.
    let origin = Origin::LocalPane {
        tab: TabKey::new(0),
        pane: PaneKey::new(0),
    };
    // The folder is the caller's destination, so links that stay inside it are made.
    let mut session = ReceiveSession::new(origin, limits).with_links(LinkPolicy::Make);
    let mut scanner = FrameScanner::new();
    let mut saved = Vec::new();
    let mut work: VecDeque<(Input, Option<FileId>)> = VecDeque::new();
    let mut wait = waits.first;
    loop {
        while let Some((input, fid)) = work.pop_front() {
            let (next, effects) = session.step(input);
            session = next;
            for effect in effects {
                match effect {
                    Effect::AskConsent(_) => work.push_back((Input::Decided(Consent::Allow), None)),
                    Effect::Reply(reply) => {
                        link.write(&encode_reply(&reply))
                            .map_err(TransferError::Link)?;
                    }
                    Effect::Write(op) => apply(root, &op, fid.clone(), &mut work),
                    Effect::Saved(file) => saved.push(file),
                    Effect::Ended(why) => return end(why, saved),
                }
            }
        }
        let bytes = link
            .read(wait)
            .map_err(TransferError::Link)?
            .ok_or(TransferError::NoAnswer)?;
        wait = waits.next;
        for payload in scanner.push(&bytes) {
            // A status about the request itself is the other side answering no.
            if let Ok(reply) = parse_reply(&payload, limits)
                && reply.id == request.id
            {
                match reply.status {
                    ReplyStatus::Failed { message, .. } => {
                        return Err(TransferError::Declined { message });
                    }
                    ReplyStatus::Cancelled => return Err(declined("the request was cancelled")),
                    ReplyStatus::Ok | ReplyStatus::Started => {}
                }
                continue;
            }
            if let Some(input) = frame_input(&payload, &request.id, limits) {
                let fid = match &input {
                    Input::Frame(message) => message.file.clone(),
                    _ => None,
                };
                work.push_back((input, fid));
            }
        }
    }
}

/// Performs one write and queues what the machine must hear back.
fn apply(
    root: &Root,
    op: &WriteOp,
    fid: Option<FileId>,
    work: &mut VecDeque<(Input, Option<FileId>)>,
) {
    match apply_write(root, op) {
        Ok(()) => {
            if let (WriteOp::Finish { .. }, Some(fid)) = (op, fid) {
                work.push_back((Input::WriteDone { fid: fid.clone() }, Some(fid)));
            }
        }
        Err(err) => {
            let kind = match op {
                WriteOp::MakeDir(_) | WriteOp::SetDirMode { .. } | WriteOp::Symlink { .. } => {
                    WriteKind::MakeDir
                }
                WriteOp::CreateFile(_) => WriteKind::Create,
                WriteOp::Append { .. } => WriteKind::Append,
                WriteOp::Finish { .. } => WriteKind::Finish,
                WriteOp::Discard(_) => WriteKind::Discard,
            };
            let input = Input::WriteFailed {
                fid,
                op: kind,
                error: wire_code(&err),
            };
            work.push_back((input, None));
        }
    }
}

fn frame_input(payload: &[u8], id: &KittyId, limits: Limits) -> Option<Input> {
    match parse_message(payload, limits) {
        Ok(message) if message.id == *id => Some(Input::Frame(message)),
        Ok(_) => None,
        Err(_) => readable_header(payload, limits)
            .filter(|found| found == id)
            .map(|id| Input::Malformed { id }),
    }
}

fn end(why: EndWhy, saved: Vec<SavedFile>) -> Result<Vec<SavedFile>, TransferError> {
    let message = match why {
        EndWhy::Finished => return Ok(saved),
        EndWhy::Refused => "the other side did not allow it",
        EndWhy::Cancelled => "the transfer was cancelled",
        _ => "the transfer broke off",
    };
    Err(declined(message))
}

fn declined(message: &str) -> TransferError {
    TransferError::Declined {
        message: message.to_owned(),
    }
}
