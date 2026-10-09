//! Joins multipart pieces into a file. The step is pure; [`Osc1337Reader`] holds its state.

use crate::base64;
use crate::error::{Malformed, TransferError};
use crate::units::{Bytes, Limits};

use super::model::{
    Completeness, Delivery, FileHeader, FileOffer, InlineImage, Osc1337Command, ReceivedFile,
};
use super::parse::parse_command;

/// Where a multipart file is: nothing open, or a header waiting for its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Assembly {
    Idle,
    Collecting { header: FileHeader, text: Vec<u8> },
}

/// Feeds OSC 1337 payloads in order and returns each file as it completes. Holds the pieces of
/// one multipart file at a time; the caller keeps one reader per pane.
#[derive(Debug)]
pub struct Osc1337Reader {
    limits: Limits,
    state: Assembly,
}

impl Osc1337Reader {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            state: Assembly::Idle,
        }
    }

    /// Parses one payload and folds it in. Returns the file when one completes: a whole `File=`
    /// or the `FileEnd` of a multipart file. Any error leaves the reader idle, so the next
    /// sequence starts clean.
    pub fn feed(
        &mut self,
        payload: &[u8],
        completeness: Completeness,
    ) -> Result<Option<ReceivedFile>, TransferError> {
        let state = std::mem::replace(&mut self.state, Assembly::Idle);
        let (next, file) = parse_command(payload, completeness, self.limits)
            .and_then(|command| step(state, command, self.limits))?;
        self.state = next;
        Ok(file)
    }
}

/// The step: `(state, command) -> (state, completed file?)`.
pub(crate) fn step(
    state: Assembly,
    command: Osc1337Command,
    limits: Limits,
) -> Result<(Assembly, Option<ReceivedFile>), TransferError> {
    match (state, command) {
        (Assembly::Idle, Osc1337Command::Whole { header, data }) => {
            Ok((Assembly::Idle, Some(file(header, data))))
        }
        (Assembly::Collecting { .. }, Osc1337Command::Whole { .. }) => {
            Err(Malformed::WholeWhileOpen.into())
        }
        (Assembly::Idle, Osc1337Command::MultipartStart(header)) => Ok((
            Assembly::Collecting {
                header,
                text: Vec::new(),
            },
            None,
        )),
        (Assembly::Collecting { .. }, Osc1337Command::MultipartStart(_)) => {
            Err(Malformed::StartWhileOpen.into())
        }
        (Assembly::Collecting { header, mut text }, Osc1337Command::Part(part)) => {
            let cap = text_cap(limits);
            if (text.len() + part.len()) as u64 > cap.get() {
                return Err(TransferError::TooLarge {
                    limit: limits.max_bytes(),
                });
            }
            text.extend_from_slice(&part);
            Ok((Assembly::Collecting { header, text }, None))
        }
        (Assembly::Idle, Osc1337Command::Part(_)) => Err(Malformed::PartWithoutStart.into()),
        (Assembly::Collecting { header, text }, Osc1337Command::End) => {
            let data = base64::decode_capped(&text, limits.max_bytes())?;
            if let Some(declared) = header.declared_size {
                let actual = Bytes::new(data.len() as u64);
                if declared != actual {
                    return Err(Malformed::SizeMismatch { declared, actual }.into());
                }
            }
            Ok((Assembly::Idle, Some(file(header, data))))
        }
        (Assembly::Idle, Osc1337Command::End) => Err(Malformed::EndWithoutStart.into()),
    }
}

/// The most base64 text a multipart file may carry: the decoded cap in base64 (4 characters per
/// 3 bytes) plus slack for line breaks.
fn text_cap(limits: Limits) -> Bytes {
    let groups = limits.max_bytes().get().div_ceil(3);
    Bytes::new(groups.saturating_mul(4).saturating_add(4096))
}

fn file(header: FileHeader, data: Vec<u8>) -> ReceivedFile {
    match header.delivery {
        Delivery::Offer => {
            ReceivedFile::NeedsConsent(FileOffer::new(header.name, header.mime, data))
        }
        Delivery::Inline => ReceivedFile::Inline(InlineImage {
            name: header.name,
            mime: header.mime,
            width: header.width,
            height: header.height,
            aspect: header.aspect,
            data,
        }),
    }
}
