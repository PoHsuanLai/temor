//! The Spaces between the two windows. The kit's handle belongs to the main window (its signals
//! live in that window's VirtualDom), so the Settings window never touches it: the main window
//! publishes what it holds of each Space here, and takes the changes the Settings window sends,
//! applying them through the handle the way a Space menu's edits go. A root context both windows
//! are given.

use std::sync::Arc;

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
    /// The Spaces as last published; the Settings window listens.
    rows: watch::Sender<Vec<SpaceRow>>,
    /// The edits sent and not yet taken; the main window listens.
    edits: watch::Sender<Vec<SpaceEdit>>,
}

/// The shared board.
#[derive(Clone)]
pub struct SpaceBoard(Arc<Inner>);

impl Default for SpaceBoard {
    fn default() -> Self {
        SpaceBoard(Arc::new(Inner {
            rows: watch::channel(Vec::new()).0,
            edits: watch::channel(Vec::new()).0,
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
        self.0.rows.send_if_modified(|held| {
            let changed = *held != rows;
            if changed {
                *held = rows;
            }
            changed
        });
    }

    /// The Spaces as last published.
    pub fn rows(&self) -> Vec<SpaceRow> {
        self.0.rows.borrow().clone()
    }

    /// The Spaces, as the Settings window hears them published.
    pub fn heard_rows(&self) -> watch::Receiver<Vec<SpaceRow>> {
        self.0.rows.subscribe()
    }

    /// The Settings window's change, for the main window to apply.
    pub fn send(&self, edit: SpaceEdit) {
        self.0.edits.send_modify(|edits| edits.push(edit));
    }

    /// What the main window hears a send through.
    pub fn heard_edits(&self) -> watch::Receiver<Vec<SpaceEdit>> {
        self.0.edits.subscribe()
    }

    /// The changes sent and not yet taken, oldest first. Taking them is not heard as a send.
    pub fn take_edits(&self) -> Vec<SpaceEdit> {
        let mut taken = Vec::new();
        self.0.edits.send_if_modified(|edits| {
            taken = std::mem::take(edits);
            false
        });
        taken
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
    fn a_publish_is_heard_only_when_it_changes_something_and_edits_come_in_order_once() {
        let board = SpaceBoard::default();
        let mut heard = board.heard_rows();
        board.publish(vec![row(1, "Home")]);
        assert!(heard.has_changed().expect("open"));
        heard.mark_unchanged();
        board.publish(vec![row(1, "Home")]);
        assert!(!heard.has_changed().expect("open"));
        board.publish(vec![row(1, "Ops")]);
        assert_eq!(board.rows()[0].name, "Ops");

        let mut sent = board.heard_edits();
        for root in ["/a", "/b"] {
            board.send(SpaceEdit {
                space: SpaceId(1),
                change: SpaceChange::Root(Some(temor_client::model::vocab::text::ProjectRoot(
                    root.to_owned(),
                ))),
            });
        }
        assert!(sent.has_changed().expect("open"));
        sent.mark_unchanged();
        let taken = board.take_edits();
        assert_eq!(taken.len(), 2);
        assert!(!sent.has_changed().expect("open"), "a take is not a send");
        assert!(board.take_edits().is_empty());
    }
}
