//! Sends files over a link with a `SendSession`.

use std::collections::VecDeque;
use std::fs::File;
use std::path::PathBuf;

use super::{Link, Waits};
use crate::error::TransferError;
use crate::kitty::frames::FrameScanner;
use crate::kitty::model::KittyId;
use crate::kitty::send::{SendEffect, SendEnd, SendInput, SendSession, describe, read_chunk};
use crate::kitty::wire::{encode_message, parse_reply};
use crate::units::Limits;

/// Offers `paths` under `id`, streams them and waits until the other side says they arrived.
/// A folder or a missing file fails before anything is sent.
pub fn drive_send(
    link: &mut impl Link,
    id: KittyId,
    paths: &[PathBuf],
    limits: Limits,
    waits: Waits,
) -> Result<(), TransferError> {
    let files = paths
        .iter()
        .map(|path| describe(path))
        .collect::<Result<Vec<_>, _>>()?;
    let (mut session, first) = SendSession::new(id, files);
    let mut effects: VecDeque<SendEffect> = first.into();
    let mut inputs: VecDeque<SendInput> = VecDeque::new();
    let mut scanner = FrameScanner::new();
    let mut open: Option<(usize, File)> = None;
    let mut wait = waits.first;
    loop {
        while let Some(effect) = effects.pop_front() {
            match effect {
                SendEffect::Emit(message) => {
                    link.write(&encode_message(&message))
                        .map_err(TransferError::Link)?;
                }
                SendEffect::Read { file, max } => {
                    let path = &paths[file];
                    if open.as_ref().is_none_or(|(index, _)| *index != file) {
                        let handle = File::open(path).map_err(|source| TransferError::Io {
                            path: path.clone(),
                            source,
                        })?;
                        open = Some((file, handle));
                    }
                    if let Some((_, handle)) = open.as_mut() {
                        let (data, end) = read_chunk(handle, max, path)?;
                        inputs.push_back(SendInput::Chunk { data, end });
                    }
                }
                SendEffect::Ended(end) => return finish(end),
            }
        }
        if let Some(input) = inputs.pop_front() {
            let (next, out) = session.step(input);
            session = next;
            effects.extend(out);
            continue;
        }
        let bytes = link
            .read(wait)
            .map_err(TransferError::Link)?
            .ok_or(TransferError::NoAnswer)?;
        wait = waits.next;
        for payload in scanner.push(&bytes) {
            if let Ok(reply) = parse_reply(&payload, limits) {
                inputs.push_back(SendInput::Reply(reply));
            }
        }
    }
}

fn finish(end: SendEnd) -> Result<(), TransferError> {
    match end {
        SendEnd::Delivered => Ok(()),
        SendEnd::Refused { message } | SendEnd::Failed { message } => {
            Err(TransferError::Declined { message })
        }
        SendEnd::Cancelled => Err(TransferError::Declined {
            message: "the transfer was cancelled".to_owned(),
        }),
    }
}
