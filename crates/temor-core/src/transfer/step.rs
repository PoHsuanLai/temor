//! `transfer_step`: one incoming file, from the offer to the shelf. A remote host's offer asks
//! the person once per host; what they answer is remembered.

use super::model::{Origin, Transfer};
use crate::remote::Grant;
use crate::vocab::ids::Bytes;

/// What moves a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransferIn {
    /// The person answered the offer.
    Answered(Grant),
    /// A frame of data arrived.
    Chunk(Bytes),
    /// The sender said it is complete.
    Completed,
    /// A frame was malformed or the stream ended early.
    Failed,
    /// The person cancelled it.
    Cancelled,
}

/// What a transfer wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransferOut {
    /// Ask the person whether to accept files from this host.
    AskPerson,
    /// Remember the answer for this host.
    Remember(Grant),
    /// Tell the sender to go ahead.
    Accept,
    /// Tell the sender no.
    Refuse,
    /// The file is on the shelf.
    Shelved,
    /// Remove the partial file.
    Discard,
}

/// A new transfer from `origin`. Files from this computer are accepted; a host's are accepted,
/// refused or put to the person by its remembered `consent`.
pub fn offer(origin: &Origin, consent: Grant) -> (Transfer, Vec<TransferOut>) {
    let receiving = (
        Transfer::Receiving { received: Bytes(0) },
        vec![TransferOut::Accept],
    );
    match (origin, consent) {
        (Origin::Local { .. } | Origin::Pipe, _) | (Origin::Remote { .. }, Grant::Allowed) => {
            receiving
        }
        (Origin::Remote { .. }, Grant::Refused) => (Transfer::Refused, vec![TransferOut::Refuse]),
        (Origin::Remote { .. }, Grant::Ask) => (Transfer::Offered, vec![TransferOut::AskPerson]),
    }
}

/// The transfer after `input`.
pub fn transfer_step(state: Transfer, input: TransferIn) -> (Transfer, Vec<TransferOut>) {
    use TransferIn as I;
    match (state, input) {
        (Transfer::Offered, I::Answered(Grant::Allowed)) => (
            Transfer::Receiving { received: Bytes(0) },
            vec![TransferOut::Remember(Grant::Allowed), TransferOut::Accept],
        ),
        (Transfer::Offered, I::Answered(Grant::Refused)) => (
            Transfer::Refused,
            vec![TransferOut::Remember(Grant::Refused), TransferOut::Refuse],
        ),
        (Transfer::Offered, I::Cancelled) => (Transfer::Refused, vec![TransferOut::Refuse]),
        (Transfer::Receiving { received }, I::Chunk(more)) => (
            Transfer::Receiving {
                received: Bytes(received.0.saturating_add(more.0)),
            },
            vec![],
        ),
        (Transfer::Receiving { .. }, I::Completed) => (Transfer::Saved, vec![TransferOut::Shelved]),
        (Transfer::Receiving { .. }, I::Failed | I::Cancelled) => {
            (Transfer::Broken, vec![TransferOut::Discard])
        }
        (state, _) => (state, vec![]),
    }
}
