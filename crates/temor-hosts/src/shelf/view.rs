//! What the shelf shows, as data: rows, how far each got and the actions each allows. The view
//! draws it and words it; nothing here knows about the screen or its language.

use super::model::{FailWhy, ItemState, Seen, Shelf, ShelfItem};
use crate::transfer::Origin;
use temor_core::vocab::ids::{Bytes, DownloadId, TabId};
use temor_core::vocab::text::ShelfPath;

/// What the person can do with a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowAction {
    Open,
    /// Show the file in the file manager.
    Reveal,
    /// Give the file to the pane the person was typing in: its path typed there, or the file
    /// sent there when that pane is on another computer (as a drop on the pane does).
    UseInPane,
    /// Put the file's path on the clipboard.
    CopyPath,
    /// Jump back to the tab it came from.
    ShowInTab(TabId),
    /// Take it off the shelf (the file stays).
    Clear,
}

/// How a row looks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowState {
    Waiting,
    /// A bar. `percent` is `None` when the sender gave no size.
    Receiving {
        percent: Option<u8>,
    },
    Ready,
    Failed {
        why: FailWhy,
    },
}

/// One row of the shelf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: DownloadId,
    /// The file name as saved.
    pub title: String,
    /// Its size: the whole file's when known, else what came so far; none while it waits or
    /// after it failed.
    pub size: Option<Bytes>,
    /// Where it came from.
    pub origin: Origin,
    pub state: RowState,
    pub actions: Vec<RowAction>,
    /// Where the file is, for the thumbnail and drag-out.
    pub path: ShelfPath,
    pub seen: Seen,
}

/// The whole shelf, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfView {
    pub rows: Vec<Row>,
    /// Files that are saved and not yet opened: the badge on the shelf button.
    pub unseen: usize,
}

/// The view of `shelf`.
pub fn shelf_view(shelf: &Shelf) -> ShelfView {
    let rows: Vec<Row> = shelf.items().iter().map(row).collect();
    let unseen = shelf
        .items()
        .iter()
        .filter(|item| matches!(item.state, ItemState::Ready { .. }) && item.seen == Seen::New)
        .count();
    ShelfView { rows, unseen }
}

fn row(item: &ShelfItem) -> Row {
    let (state, size) = match item.state {
        ItemState::Waiting => (RowState::Waiting, None),
        ItemState::Receiving { received, total } => (
            RowState::Receiving {
                percent: total.and_then(|total| percent(received, total)),
            },
            total.or(Some(received)),
        ),
        ItemState::Ready { size } => (RowState::Ready, Some(size)),
        ItemState::Failed { why } => (RowState::Failed { why }, None),
    };
    let mut actions = Vec::new();
    if matches!(item.state, ItemState::Ready { .. }) {
        actions.push(RowAction::Open);
        actions.push(RowAction::Reveal);
        actions.push(RowAction::UseInPane);
        actions.push(RowAction::CopyPath);
    }
    if let Some(tab) = tab_of(&item.origin) {
        actions.push(RowAction::ShowInTab(tab));
    }
    if !matches!(item.state, ItemState::Waiting | ItemState::Receiving { .. }) {
        actions.push(RowAction::Clear);
    }
    Row {
        id: item.id,
        title: item.name_as_saved(),
        size,
        origin: item.origin.clone(),
        state,
        actions,
        path: item.path.clone(),
        seen: item.seen,
    }
}

impl ShelfItem {
    /// The last component of the path: the name with any " (2)" added.
    fn name_as_saved(&self) -> String {
        let path = self.path.as_str();
        path.rsplit('/').next().unwrap_or(path).to_owned()
    }
}

fn tab_of(origin: &Origin) -> Option<TabId> {
    match origin {
        Origin::Remote { tab, .. } | Origin::LocalStream { tab } | Origin::Local { tab, .. } => {
            Some(*tab)
        }
        Origin::Pipe => None,
    }
}

/// Whole percent, 0 to 100. `None` when the total is zero.
fn percent(received: Bytes, total: Bytes) -> Option<u8> {
    if total.0 == 0 {
        return None;
    }
    let scaled = u128::from(received.0).saturating_mul(100) / u128::from(total.0);
    Some(u8::try_from(scaled.min(100)).unwrap_or(100))
}
