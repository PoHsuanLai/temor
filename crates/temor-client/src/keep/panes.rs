//! What is kept of each pane for the next launch, beside the layout: its checkpoint (its shell's
//! state, the daemon session it has) and, at quit, its screen. Files keyed by pane, good for the
//! launch that follows the quit that wrote them: pane ids are reused after a restore. A pane in a
//! history session keeps its screen under the session instead ([`screens`]), for as long as
//! history keeps the session.

use std::collections::BTreeMap;
use std::path::Path;

use temor_blocks::session::Checkpoint;
use temor_core::vocab::ids::PaneId;

use temor_schema::session::SessionId;

use super::{checkpoint, replay, screens, sidecar};

/// What is kept of one pane.
#[derive(Debug)]
pub struct KeptPane {
    pub pane: PaneId,
    pub checkpoint: Checkpoint,
    /// Its screen as the terminal encodes it, taken only at quit (encoding it is not free).
    pub screen: Option<Vec<u8>>,
    /// Its session in history, once it has one: its screen is kept under it.
    pub session: Option<SessionId>,
}

/// The panes' files, and what each pane's checkpoint file holds now, so only a change is written.
#[derive(Debug, Default)]
pub struct PaneFiles {
    on_disk: BTreeMap<PaneId, Checkpoint>,
}

impl PaneFiles {
    /// Writes the checkpoint of each pane whose state changed since it was last written, so a
    /// quit that never reaches the window's end still leaves the shells' state for the next launch.
    pub fn keep_changed(&mut self, dir: &Path, panes: Vec<KeptPane>) {
        for kept in panes {
            if self.on_disk.get(&kept.pane) != Some(&kept.checkpoint)
                && checkpoint::write(dir, kept.pane, &kept.checkpoint).is_ok()
            {
                self.on_disk.insert(kept.pane, kept.checkpoint);
            }
        }
    }

    /// Writes each pane's checkpoint, changed or not.
    pub fn keep_all(dir: &Path, panes: &[KeptPane]) {
        for kept in panes {
            let _ = checkpoint::write(dir, kept.pane, &kept.checkpoint);
        }
    }

    /// Writes what a quit keeps, in place of what an earlier quit left: ids restart from the live
    /// maximum, so an older file must not outlive this quit.
    pub fn keep_at_quit(dir: &Path, panes: &[KeptPane]) {
        sidecar::clear(dir);
        checkpoint::clear(dir);
        replay::clear(dir);
        for kept in panes {
            let _ = checkpoint::write(dir, kept.pane, &kept.checkpoint);
            match (&kept.screen, kept.session) {
                (Some(screen), Some(id)) => {
                    let _ = screens::write(dir, id, screen);
                }
                (Some(screen), None) => {
                    let _ = sidecar::write(dir, kept.pane, screen);
                }
                (None, _) => {}
            }
        }
    }

    /// The screen and the checkpoint kept for `pane`, each deleted as it is read.
    pub fn take(dir: &Path, pane: PaneId) -> (Option<Vec<u8>>, Option<Checkpoint>) {
        (sidecar::take(dir, pane), checkpoint::take(dir, pane))
    }

    /// Deletes what no pane took once a launch has started its panes: it is good for one launch.
    pub fn clear(dir: &Path) {
        sidecar::clear(dir);
        checkpoint::clear(dir);
    }

    /// A pane that can no longer come back: its files go.
    pub fn forget(&mut self, dir: Option<&Path>, pane: PaneId) {
        if let Some(dir) = dir {
            sidecar::remove(dir, pane);
            checkpoint::remove(dir, pane);
        }
        self.on_disk.remove(&pane);
    }
}
