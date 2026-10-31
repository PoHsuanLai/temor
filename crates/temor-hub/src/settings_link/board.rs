//! The Spaces between the two windows. The kit's handle belongs to the main window (its signals
//! live in that window's VirtualDom), so the Settings window never touches it: the main window
//! publishes what it holds of each Space here, and takes the changes the Settings window sends,
//! applying them through the handle the way a Space menu's edits go. A root context both windows
//! are given.

use std::sync::{Arc, Mutex, PoisonError};

use ds::components::app::spaces::SpaceId;
use temor_client::model::space::Project;
use tokio::sync::watch;

use super::space_edit::SpaceChange;

/// One Space as the Settings window shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceRow {
    pub id: SpaceId,
    pub name: String,
    pub project: Project,
}

/// A change to a Space, on its way to the main window.
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceEdit {
    pub space: SpaceId,
    pub change: SpaceChange,
}

struct Inner {
    rows: Mutex<Vec<SpaceRow>>,
    edits: Mutex<Vec<SpaceEdit>>,
    /// Counts publishes that changed something; the Settings window listens.
    shown: watch::Sender<u64>,
    /// Counts edits sent; the main window listens.
    sent: watch::Sender<u64>,
}

/// The shared board.
#[derive(Clone)]
pub struct SpaceBoard(Arc<Inner>);

impl Default for SpaceBoard {
    fn default() -> Self {
        SpaceBoard(Arc::new(Inner {
            rows: Mutex::new(Vec::new()),
            edits: Mutex::new(Vec::new()),
            shown: watch::channel(0).0,
            sent: watch::channel(0).0,
        }))
    }
}

impl std::fmt::Debug for SpaceBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpaceBoard")
    }
}

impl SpaceBoard {
    /// The main window's Spaces as they are now. Listeners hear only a change.
    pub fn publish(&self, rows: Vec<SpaceRow>) {
        let mut held = self.0.rows.lock().unwrap_or_else(PoisonError::into_inner);
        if *held != rows {
            *held = rows;
            self.0.shown.send_modify(|count| *count += 1);
        }
    }

    /// The Spaces as last published.
    pub fn rows(&self) -> Vec<SpaceRow> {
        self.0
            .rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// What the Settings window hears a publish through.
    pub fn heard_rows(&self) -> watch::Receiver<u64> {
        self.0.shown.subscribe()
    }

    /// The Settings window's change, for the main window to apply.
    pub fn send(&self, edit: SpaceEdit) {
        self.0
            .edits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(edit);
        self.0.sent.send_modify(|count| *count += 1);
    }

    /// What the main window hears a send through.
    pub fn heard_edits(&self) -> watch::Receiver<u64> {
        self.0.sent.subscribe()
    }

    /// The changes sent and not yet taken, oldest first.
    pub fn take_edits(&self) -> Vec<SpaceEdit> {
        std::mem::take(&mut *self.0.edits.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u64, name: &str) -> SpaceRow {
        SpaceRow {
            id: SpaceId(id),
            name: name.to_owned(),
            project: Project::default(),
        }
    }

    #[test]
    fn a_publish_is_heard_only_when_it_changes_something_and_edits_come_in_order() {
        let board = SpaceBoard::default();
        let mut heard = board.heard_rows();
        board.publish(vec![row(1, "Home")]);
        assert!(heard.has_changed().expect("open"));
        heard.mark_unchanged();
        board.publish(vec![row(1, "Home")]);
        assert!(!heard.has_changed().expect("open"));
        board.publish(vec![row(1, "Ops")]);
        assert_eq!(board.rows()[0].name, "Ops");

        for root in ["/a", "/b"] {
            board.send(SpaceEdit {
                space: SpaceId(1),
                change: SpaceChange::Root(Some(temor_client::model::vocab::text::ProjectRoot(
                    root.to_owned(),
                ))),
            });
        }
        let taken = board.take_edits();
        assert_eq!(taken.len(), 2);
        assert!(board.take_edits().is_empty());
    }
}
