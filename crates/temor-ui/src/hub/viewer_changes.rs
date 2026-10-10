//! The conflict question's "Show changes" (PLAN §9b): the host's file as it is now is fetched,
//! compared with the person's copy, and the differences open in a viewer beside a pane of the
//! host (see `viewer_diff`).

use std::path::Path;

use super::desk::Desk;
use super::model::Hub;
use super::ssh_edit::forget_copy;
use super::viewer_diff::{Changes, changes};
use super::viewer_host::{Then, Wanted};
use crate::edge::ssh_work::EditId;
use crate::edge::viewer_host::Cap;

/// The most of a file fetched to compare: more is not shown as a diff (see `viewer_diff`).
const DIFF_CAP: u64 = 2 * 1024 * 1024;

impl Hub {
    /// The conflict question's "Show changes": the host's file as it is now is fetched and
    /// compared with the person's copy; the differences open in a viewer beside a pane of the
    /// host.
    pub fn show_edit_changes(&mut self, id: EditId) {
        let Some(edit) = self.ssh.edits.get(&id) else {
            return;
        };
        let (host, path, yours, tier) = (
            edit.host.clone(),
            edit.path.as_str().to_owned(),
            edit.local.clone(),
            edit.tier,
        );
        let Some(from) = self.ssh.panes_on(&host).first().copied() else {
            self.notices.push(format!(
                "Open a tab on {} to see the changes",
                host.as_str()
            ));
            return;
        };
        let how = (Then::Diff { yours }, Cap::Bytes(DIFF_CAP));
        self.begin_fetch((from, host, tier), &path, how);
    }

    /// Writes the differences between the host's file `want.local` and the person's copy
    /// `yours` beside it, and opens the viewer on them.
    pub(super) fn show_changes(&mut self, want: &Wanted, yours: &Path) {
        let (theirs, mine) = (std::fs::read(&want.local), std::fs::read(yours));
        let names = (
            format!("On {} now", want.host.as_str()),
            "Your copy".to_owned(),
        );
        let found = match (&theirs, &mine) {
            (Ok(theirs), Ok(mine)) => changes(theirs, mine, (&names.0, &names.1)),
            _ => Changes::NotText,
        };
        let name = &want.name;
        let text = match found {
            Changes::Text(text) => text,
            Changes::Same => {
                self.notices
                    .push(format!("{name} reads the same in both places"));
                forget_copy(&want.local);
                return;
            }
            Changes::NotText => {
                self.notices
                    .push(format!("{name} isn't text, so its changes can't be shown"));
                forget_copy(&want.local);
                return;
            }
            Changes::TooBig => {
                self.notices
                    .push(format!("{name} is too big to show its changes"));
                forget_copy(&want.local);
                return;
            }
        };
        let diff = want.local.with_file_name(format!("{name}.diff"));
        let written = std::fs::write(&diff, text);
        // The host's file served its turn; the differences are what is shown.
        let _ = std::fs::remove_file(&want.local);
        match written {
            Ok(()) => self.show_file(want.from, diff),
            Err(err) => {
                forget_copy(&diff);
                self.notices
                    .push(format!("The changes can't be shown: {err}"));
            }
        }
    }
}

impl Desk {
    /// The conflict question's "Show changes" for the edit `id`.
    pub fn host_show_changes(&self, id: EditId) {
        let notes = {
            let mut hub = self.hub.0.borrow_mut();
            hub.show_edit_changes(id);
            std::mem::take(&mut hub.notices)
        };
        for note in notes {
            self.say(note);
        }
        self.bump();
    }
}
