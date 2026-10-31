//! The conflict question's "Show changes" (PLAN §9b): the host's file as it is now is fetched,
//! compared with the person's copy, and the differences open in a viewer beside a pane of the
//! host (see `temor_client::changes`).

use super::desk::Desk;
use super::model::Hub;
use super::ssh_edit::forget_copy;
use super::viewer_host::{Then, Wanted, diff_beside};
use temor_client::changes::Compared;
use temor_client::ssh::work::EditId;
use temor_client::viewer_host::Cap;

/// The most of a file fetched to compare: more is not shown as a diff (see `temor_client::changes`).
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

    /// The differences of the fetched `want` from the person's copy, as compared on the copy's
    /// thread: the viewer opens on them, or the person is told why there are none to show.
    pub(super) fn show_changes(&mut self, want: &Wanted, compared: Compared) {
        let name = &want.name;
        let why = match compared {
            Compared::Written => {
                return self.show_file(want.from, diff_beside(&want.local, name));
            }
            Compared::Same => format!("{name} reads the same in both places"),
            Compared::NotText => format!("{name} isn't text, so its changes can't be shown"),
            Compared::TooBig => format!("{name} is too big to show its changes"),
            Compared::Unwritten => format!("The changes to {name} can't be shown"),
        };
        self.notices.push(why);
        forget_copy(&want.local);
    }
}

impl Desk {
    /// The conflict question's "Show changes" for the edit `id`.
    pub fn host_show_changes(&self, id: EditId) {
        self.settle(|hub| hub.show_edit_changes(id));
    }
}
