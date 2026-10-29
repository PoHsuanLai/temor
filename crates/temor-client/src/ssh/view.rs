//! What the views show of a host pane (PLAN §9b, Tier A): the question being asked, whether the
//! connection is up, the browser check, the forwards and the files being edited. Plain data; the
//! window reads it from its panes and draws it.

use crate::model::host::consent::BootstrapAction;
use crate::model::host::edit::{Edit, Found};
use crate::ssh::Port;
use crate::ssh::work::EditId;

/// Where the host pane stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The person is being asked this.
    Asking(BootstrapAction),
    /// Connecting: a password, a passphrase or a browser check may be asked for in the pane.
    Connecting,
    /// Connected.
    Up,
    /// The session ended; the pane offers to start another.
    Ended,
    /// The tailnet shows the computer offline; the host view offers to try anyway.
    Offline,
    /// The session never started (the connection could not be placed): the host view offers to
    /// try again.
    Failed,
}

/// A forwarded port. `local` is the port on this computer once the forward is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardLine {
    pub remote: Port,
    pub local: Option<Port>,
}

/// What an edit is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditStatus {
    Opening,
    /// Open in the editor; saves go to the host.
    Open,
    Saving,
    /// The host's file changed since it was fetched: the person chooses.
    Changed,
    /// The host's file was written again since it was fetched, yet its time and size read the
    /// same (only the helper can tell): the person chooses.
    Rewritten,
    /// The host's file was removed since it was fetched: the person chooses.
    Deleted,
    /// A call failed; saving again tries again.
    Stalled,
    Broken,
}

/// A file of the host open for editing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditLine {
    pub id: EditId,
    /// `host:/path`.
    pub name: String,
    pub status: EditStatus,
}

/// Everything the views show for one host pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostView {
    pub host: String,
    pub stage: Stage,
    /// The host wants the person to confirm in a browser: an Open button is shown.
    pub check_waiting: bool,
    /// Whether the host's ports can be opened without a forward.
    pub direct_ports: bool,
    pub forwards: Vec<ForwardLine>,
    pub edits: Vec<EditLine>,
}

/// How an edit in `state` reads, with what the host was found to hold when it is `asking`.
pub fn status_of(state: Edit, asking: Option<Found>) -> EditStatus {
    match (state, asking) {
        (Edit::Opening, _) => EditStatus::Opening,
        (Edit::Open { .. }, _) => EditStatus::Open,
        (Edit::Checking { .. } | Edit::Uploading { .. }, _) => EditStatus::Saving,
        (Edit::Conflict { .. }, Some(Found::Deleted)) => EditStatus::Deleted,
        (Edit::Conflict { base, .. }, Some(Found::Changed(now))) if base == now => {
            EditStatus::Rewritten
        }
        (Edit::Conflict { .. }, _) => EditStatus::Changed,
        (Edit::Stalled { .. }, _) => EditStatus::Stalled,
        (Edit::Broken(_) | Edit::Closed, _) => EditStatus::Broken,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::{ByteCount, RemoteStat, UnixSeconds};

    #[test]
    fn a_conflict_reads_as_changed_rewritten_or_removed() {
        let stat = |secs, size| RemoteStat {
            mtime: UnixSeconds(secs),
            size: ByteCount(size),
        };
        let base = stat(10, 5);
        let conflict = |found| Edit::Conflict { base, found };
        let cases = [
            (
                "later time",
                Found::Changed(stat(11, 5)),
                EditStatus::Changed,
            ),
            (
                "other size",
                Found::Changed(stat(10, 6)),
                EditStatus::Changed,
            ),
            (
                "same time and size",
                Found::Changed(base),
                EditStatus::Rewritten,
            ),
            ("removed", Found::Deleted, EditStatus::Deleted),
        ];
        for (name, found, want) in cases {
            assert_eq!(status_of(conflict(found), Some(found)), want, "{name}");
        }
    }
}
