//! What the shelf shows, as data: rows with plain-language text and the actions each allows.
//! The view draws it; nothing here knows about the screen.

use super::model::{FailWhy, ItemState, Seen, Shelf, ShelfItem};
use crate::transfer::Origin;
use crate::vocab::ids::{Bytes, DownloadId, TabId};
use crate::vocab::text::ShelfPath;

/// What the person can do with a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowAction {
    Open,
    /// Show the file in the file manager.
    Reveal,
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
        message: String,
    },
}

/// One row of the shelf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: DownloadId,
    /// The file name as saved.
    pub title: String,
    /// Size and origin, such as `1.2 MB, from prod`.
    pub detail: String,
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
    /// The words shown when there are no rows.
    pub empty: &'static str,
}

/// The view of `shelf`.
pub fn shelf_view(shelf: &Shelf) -> ShelfView {
    let rows: Vec<Row> = shelf.items().iter().map(row).collect();
    let unseen = shelf
        .items()
        .iter()
        .filter(|item| matches!(item.state, ItemState::Ready { .. }) && item.seen == Seen::New)
        .count();
    ShelfView {
        rows,
        unseen,
        empty: "Files you save or receive appear here",
    }
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
        ItemState::Failed { why } => (
            RowState::Failed {
                message: failure(why).to_owned(),
            },
            None,
        ),
    };
    let mut actions = Vec::new();
    if matches!(item.state, ItemState::Ready { .. }) {
        actions.push(RowAction::Open);
        actions.push(RowAction::Reveal);
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
        detail: detail(size, &item.origin),
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

fn failure(why: FailWhy) -> &'static str {
    match why {
        FailWhy::Refused => "You said no",
        FailWhy::Broken => "The transfer stopped before it finished",
        FailWhy::TooBig => "The file is too big",
        FailWhy::Unwritable => "temor could not save it in your Downloads folder",
    }
}

fn detail(size: Option<Bytes>, origin: &Origin) -> String {
    let from = match origin {
        Origin::Remote { host, .. } => format!("from {}", host.as_str()),
        Origin::LocalStream { .. } => "from a program in this tab".to_owned(),
        Origin::Local { .. } => "saved here".to_owned(),
        Origin::Pipe => "from a pipe".to_owned(),
    };
    match size {
        Some(size) => format!("{}, {from}", human_size(size)),
        None => from,
    }
}

/// `999 bytes`, `1.0 KB`, `12.3 MB`: powers of 1000, one decimal above bytes.
pub(super) fn human_size(size: Bytes) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if size.0 < 1000 {
        return match size.0 {
            1 => "1 byte".to_owned(),
            n => format!("{n} bytes"),
        };
    }
    let mut tenths = size.0 / 100; // tenths of a KB
    let mut unit = 0;
    while tenths >= 10_000 && unit + 1 < UNITS.len() {
        tenths /= 1000;
        unit += 1;
    }
    format!("{}.{} {}", tenths / 10, tenths % 10, UNITS[unit])
}
