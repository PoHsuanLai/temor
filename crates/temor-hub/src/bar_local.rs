//! A local tab from a host pane. A new tab made while a host pane is in front opens on that
//! host, as a split does (`ssh_prepare`); the bar lists "New tab on this computer" beside it, so
//! this computer's own shell is one pick away (`Target::LocalTab`, a tab in the home folder: a
//! tab given a folder is never taken to the host).

use temor_client::model::bar::{Candidate, Target};

use super::model::Hub;

/// What the row says.
pub(super) const LOCAL_TAB_TITLE: &str = "New tab on this computer";

impl Hub {
    /// The row that opens a tab on this computer, only while the pane in front is a session on a
    /// host (anywhere else a plain new tab is already this computer's).
    pub(super) fn local_tab_candidate(&self) -> Option<Candidate> {
        let pane = self.keyboard_pane()?;
        if !self.ssh.panes.contains_key(&pane) {
            return None;
        }
        Some(Candidate {
            title: LOCAL_TAB_TITLE.to_owned(),
            target: Target::LocalTab,
        })
    }
}
