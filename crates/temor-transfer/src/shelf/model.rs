//! The shelf's pure model: an ordered list of items, newest first.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::name::FileName;
use crate::origin::Origin;
use crate::units::Bytes;

/// An item's id, unique within one shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ShelfId(u64);

impl ShelfId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A point in time, in milliseconds since the UNIX epoch. Supplied by the caller's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnixMillis(i64);

impl UnixMillis {
    pub const fn new(millis: i64) -> Self {
        Self(millis)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

/// One received file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShelfItem {
    pub id: ShelfId,
    pub name: FileName,
    pub path: PathBuf,
    pub size: Bytes,
    pub origin: Origin,
    pub received_at: UnixMillis,
}

/// The shelf: newest first. Adding keeps that order by `received_at`, so an item with an older
/// clock reading never jumps ahead of a newer one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shelf {
    items: Vec<ShelfItem>,
}

impl Shelf {
    pub fn items(&self) -> &[ShelfItem] {
        &self.items
    }

    pub fn find(&self, id: ShelfId) -> Option<&ShelfItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// The id one past the largest in use, so ids are never reused while the shelf lives.
    pub fn next_id(&self) -> ShelfId {
        let largest = self
            .items
            .iter()
            .map(|item| item.id.get())
            .max()
            .unwrap_or(0);
        ShelfId::new(largest + 1)
    }

    /// Adds an item in its place by time.
    pub fn insert_item(self, item: ShelfItem) -> Self {
        let mut items = self.items;
        let at = items
            .iter()
            .position(|existing| existing.received_at < item.received_at)
            .unwrap_or(items.len());
        items.insert(at, item);
        Self { items }
    }

    /// Removes an item by id, returning the shelf without it and the item if it was there.
    pub fn remove(self, id: ShelfId) -> (Self, Option<ShelfItem>) {
        let mut items = self.items;
        let found = items
            .iter()
            .position(|item| item.id == id)
            .map(|at| items.remove(at));
        (Self { items }, found)
    }
}
