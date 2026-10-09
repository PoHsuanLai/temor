//! `shelf_step`: what arrives, how far it got, and what the person does with it.

use super::model::{Clear, FailWhy, Incoming, ItemState, Seen, Shelf, ShelfItem};
use super::naming::safe_name;
use crate::vocab::ids::{Bytes, DownloadId};
use crate::vocab::text::ShelfPath;

/// What moves the shelf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfIn {
    /// A file was accepted (kitty, `File=` or `temor save`).
    Incoming(Incoming),
    /// The edge found the name already in the folder. The shelf picks the next one.
    NameTaken { id: DownloadId },
    /// The edge created the file; data is arriving.
    Progress { id: DownloadId, received: Bytes },
    /// The file is complete and saved.
    Done { id: DownloadId, size: Bytes },
    /// The file did not arrive.
    Failed { id: DownloadId, why: FailWhy },
    /// The person opened it.
    Opened { id: DownloadId },
    /// The person asked to see it in the file manager.
    Revealed { id: DownloadId },
    /// The person cleared items from the shelf.
    Cleared(Clear),
}

/// What the shelf wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfOut {
    /// Create this file, and only if it does not exist yet (exclusive create). If it does,
    /// answer `ShelfIn::NameTaken`. Nothing already in the folder is ever overwritten.
    Place { id: DownloadId, path: ShelfPath },
    /// Open the file with the system's opener.
    Open(ShelfPath),
    /// Show the file in the file manager.
    Reveal(ShelfPath),
    /// A new item appeared: the window can show the shelf's badge.
    Announce(DownloadId),
}

/// The shelf after `input`. An id the shelf does not know changes nothing.
pub fn shelf_step(shelf: Shelf, input: ShelfIn) -> (Shelf, Vec<ShelfOut>) {
    match input {
        ShelfIn::Incoming(incoming) => incoming_item(shelf, incoming),
        ShelfIn::NameTaken { id } => name_taken(shelf, id),
        ShelfIn::Progress { id, received } => update(shelf, id, |item| match item.state {
            ItemState::Waiting => Some(ItemState::Receiving {
                received,
                total: None,
            }),
            ItemState::Receiving { total, .. } => Some(ItemState::Receiving { received, total }),
            _ => None,
        }),
        ShelfIn::Done { id, size } => update(shelf, id, |item| match item.state {
            ItemState::Waiting | ItemState::Receiving { .. } => Some(ItemState::Ready { size }),
            _ => None,
        }),
        ShelfIn::Failed { id, why } => update(shelf, id, |item| match item.state {
            ItemState::Waiting | ItemState::Receiving { .. } => Some(ItemState::Failed { why }),
            _ => None,
        }),
        ShelfIn::Opened { id } => on_ready(shelf, id, Seen::Opened, ShelfOut::Open),
        ShelfIn::Revealed { id } => on_ready(shelf, id, Seen::New, ShelfOut::Reveal),
        ShelfIn::Cleared(what) => (clear(shelf, what), Vec::new()),
    }
}

fn incoming_item(shelf: Shelf, incoming: Incoming) -> (Shelf, Vec<ShelfOut>) {
    let name = safe_name(&incoming.name);
    let attempt = shelf.free_attempt(&name, 0);
    let path = shelf.path_for(&name, attempt);
    let (id, mut shelf) = shelf.id_and_next();
    let state = match incoming.size {
        Some(total) => ItemState::Receiving {
            received: Bytes(0),
            total: Some(total),
        },
        None => ItemState::Waiting,
    };
    shelf.items.insert(
        0,
        ShelfItem {
            id,
            name,
            path: path.clone(),
            origin: incoming.origin,
            source: incoming.source,
            at: incoming.at,
            state,
            attempt,
            seen: Seen::New,
        },
    );
    (
        shelf,
        vec![ShelfOut::Announce(id), ShelfOut::Place { id, path }],
    )
}

fn name_taken(shelf: Shelf, id: DownloadId) -> (Shelf, Vec<ShelfOut>) {
    let Some(at) = shelf.items.iter().position(|item| item.id == id) else {
        return (shelf, Vec::new());
    };
    let mut shelf = shelf;
    let name = shelf.items[at].name.clone();
    let from = shelf.items[at].attempt.saturating_add(1);
    // The item's own path must not count as taken by someone else.
    let attempt = (from..)
        .find(|&n| {
            let path = shelf.path_for(&name, n);
            !shelf
                .items
                .iter()
                .any(|other| other.id != id && other.path == path)
        })
        .unwrap_or(from);
    let path = shelf.path_for(&name, attempt);
    let item = &mut shelf.items[at];
    item.attempt = attempt;
    item.path = path.clone();
    (shelf, vec![ShelfOut::Place { id, path }])
}

fn update(
    shelf: Shelf,
    id: DownloadId,
    next: impl FnOnce(&ShelfItem) -> Option<ItemState>,
) -> (Shelf, Vec<ShelfOut>) {
    let mut shelf = shelf;
    if let Some(item) = shelf.items.iter_mut().find(|item| item.id == id)
        && let Some(state) = next(item)
    {
        item.state = state;
    }
    (shelf, Vec::new())
}

fn on_ready(
    shelf: Shelf,
    id: DownloadId,
    seen: Seen,
    out: fn(ShelfPath) -> ShelfOut,
) -> (Shelf, Vec<ShelfOut>) {
    let mut shelf = shelf;
    let Some(item) = shelf.items.iter_mut().find(|item| item.id == id) else {
        return (shelf, Vec::new());
    };
    if !matches!(item.state, ItemState::Ready { .. }) {
        return (shelf, Vec::new());
    }
    if seen == Seen::Opened {
        item.seen = Seen::Opened;
    }
    let path = item.path.clone();
    (shelf, vec![out(path)])
}

fn clear(shelf: Shelf, what: Clear) -> Shelf {
    let mut shelf = shelf;
    shelf.items.retain(|item| {
        let arriving = matches!(item.state, ItemState::Waiting | ItemState::Receiving { .. });
        match what {
            Clear::One(id) => item.id != id,
            Clear::Finished => arriving,
            Clear::All => false,
        }
    });
    shelf
}
