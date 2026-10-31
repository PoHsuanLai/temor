//! The downloads shelf at the edge (PLAN §9): placing a staged file in the Downloads folder
//! without overwriting anything, keeping the shelf's index on disk through `ShelfStore`, and
//! listing what the shelf can hand to a program that asks for a file.
//!
//! The live model is temor-core's `Shelf`; the index is temor-transfer's `Shelf`. They are mapped
//! here, so the index keeps the facts a restart needs (name, place, size, where from, when).

/// The transfers' own names (consent, file names, sizes and limits, the shelf's index), for the
/// window: only these, not the transfer engine.
pub mod transfer {
    pub use temor_transfer::{Bytes, Consent, Count, FileName, Limits, shelf};
}

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use ds_core::time::stamp::Stamp;
use temor_core::vocab::ids::{BlockId, Bytes, DownloadId, TabId};
use temor_core::vocab::text::{HostName, ShelfPath};
use temor_hosts::shelf::{Incoming, ItemState, Shelf, ShelfIn, ShelfItem, Source, shelf_step};
use temor_hosts::transfer::Origin;
use temor_transfer::kitty::Offered;
use temor_transfer::shelf::{Clock, ShelfId, ShelfStore, UnixMillis};
use temor_transfer::{
    BlockKey, FileName, HostName as StoredHost, Origin as StoredOrigin, PaneKey, TabKey,
};

/// Saves `text` temor made (a block's output) on the shelf in `dir` under `name`, or the next
/// free name: the file is created exclusively, so a link planted at the name is never followed.
pub fn save_text(
    dir: &Path,
    text: &str,
    name: &FileName,
) -> Result<PathBuf, temor_transfer::TransferError> {
    ShelfStore::new(dir.to_path_buf()).place_text(text, name)
}

/// Why a staged file could not be placed.
#[derive(Debug)]
pub enum PlaceError {
    /// The name is already in the folder; the shelf picks the next one.
    Taken,
    /// The folder or the file could not be written.
    Io(io::Error),
}

/// Moves the staged file to `dest`, which must not exist yet: it is created exclusively (a name
/// someone else holds, or a link planted there, is never written through), filled and synced,
/// and only then is the staged copy removed.
pub fn place(staged: &Path, dest: &Path) -> Result<(), PlaceError> {
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(PlaceError::Io)?;
    }
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|err| match err.kind() {
            io::ErrorKind::AlreadyExists => PlaceError::Taken,
            _ => PlaceError::Io(err),
        })?;
    let copied = File::open(staged)
        .and_then(|mut source| io::copy(&mut source, &mut out))
        .and_then(|_| out.sync_all());
    if let Err(err) = copied {
        drop(out);
        let _ = fs::remove_file(dest);
        return Err(PlaceError::Io(err));
    }
    // The copy is whole; the staged file and its folder are no longer needed.
    let _ = fs::remove_file(staged);
    if let Some(folder) = staged.parent() {
        let _ = fs::remove_dir(folder);
    }
    Ok(())
}

/// The shelf read back from its index: files that are still there, oldest first so the newest
/// ends on top, all counted as seen. Also when each arrived. An unreadable index comes back as
/// an empty shelf and the reason (the store has set the bad file aside).
pub fn restore(
    store: &ShelfStore,
    dir: &ShelfPath,
    clock: &impl Clock,
) -> (Shelf, BTreeMap<DownloadId, UnixMillis>, Option<String>) {
    let mut shelf = Shelf::new(dir.clone());
    let mut when = BTreeMap::new();
    let stored = match store.load(clock) {
        Ok(stored) => stored,
        Err(err) => return (shelf, when, Some(err.to_string())),
    };
    for item in stored.items().iter().rev() {
        let here = item.path.parent() == Some(Path::new(dir.as_str()));
        let name = item.path.file_name().and_then(|name| name.to_str());
        let Some(name) = name.filter(|_| here && item.path.is_file()) else {
            continue;
        };
        let (next, _) = shelf_step(
            shelf,
            ShelfIn::Incoming(Incoming {
                source: Source::Kitty,
                origin: origin_from(&item.origin),
                name: name.to_owned(),
                size: None,
                at: Stamp(0),
            }),
        );
        shelf = next;
        let Some(id) = shelf.items().first().map(|added| added.id) else {
            continue;
        };
        let size = Bytes(item.size.get());
        for step in [ShelfIn::Done { id, size }, ShelfIn::Opened { id }] {
            shelf = shelf_step(shelf, step).0;
        }
        when.insert(id, item.received_at);
    }
    (shelf, when, None)
}

/// Writes the index from the live shelf: the files that arrived whole, newest first.
pub fn persist(
    store: &ShelfStore,
    shelf: &Shelf,
    when: &BTreeMap<DownloadId, UnixMillis>,
    clock: &impl Clock,
) -> Result<(), temor_transfer::TransferError> {
    let mut stored = temor_transfer::shelf::Shelf::default();
    for item in shelf.items().iter().rev() {
        let ItemState::Ready { size } = item.state else {
            continue;
        };
        stored = stored.insert_item(temor_transfer::shelf::ShelfItem {
            id: ShelfId::new(item.id.0),
            name: FileName::sanitise(&item.name),
            path: PathBuf::from(item.path.as_str()),
            size: temor_transfer::Bytes::new(size.0),
            origin: origin_to(&item.origin),
            received_at: when.get(&item.id).copied().unwrap_or_else(|| clock.now()),
        });
    }
    store.save(&stored)
}

/// What a program that asks for a file may be given: the shelf's saved files, newest first.
pub fn offered(shelf: &Shelf) -> Vec<Offered> {
    shelf.items().iter().filter_map(offer_of).collect()
}

fn offer_of(item: &ShelfItem) -> Option<Offered> {
    let ItemState::Ready { size } = item.state else {
        return None;
    };
    Some(Offered {
        name: FileName::sanitise(&item.name),
        path: PathBuf::from(item.path.as_str()),
        size: temor_transfer::Bytes::new(size.0),
        modified: None,
    })
}

fn origin_to(origin: &Origin) -> StoredOrigin {
    match origin {
        Origin::Remote { host, .. } => StoredOrigin::Host {
            host: StoredHost::new(host.as_str()),
        },
        Origin::LocalStream { tab } => StoredOrigin::LocalPane {
            tab: TabKey::new(tab.0),
            pane: PaneKey::new(0),
        },
        Origin::Local { tab, block } => match block {
            Some(block) => StoredOrigin::Save {
                block: BlockKey::new(block.0),
            },
            None => StoredOrigin::LocalPane {
                tab: TabKey::new(tab.0),
                pane: PaneKey::new(0),
            },
        },
        Origin::Pipe => StoredOrigin::LocalPane {
            tab: TabKey::new(0),
            pane: PaneKey::new(0),
        },
    }
}

/// The tab of a stored origin is not kept across a restart for hosts and saves: "Show in its
/// tab" then names no tab (id 0) and the workspace answers that it is no longer open.
fn origin_from(origin: &StoredOrigin) -> Origin {
    match origin {
        StoredOrigin::Host { host } => Origin::Remote {
            host: HostName(host.as_str().to_owned()),
            tab: TabId(0),
        },
        StoredOrigin::LocalPane { tab, .. } => Origin::LocalStream {
            tab: TabId(tab.get()),
        },
        StoredOrigin::Save { block } => Origin::Local {
            tab: TabId(0),
            block: Some(BlockId(block.get())),
        },
    }
}
