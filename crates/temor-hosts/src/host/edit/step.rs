//! `edit_step`: one remote edit from the fetch to the write-back.

use super::model::{Edit, EditFault, EditIn, EditOut, Found, Queued, Resolution};
use temor_ssh::RemoteState;

/// The edit after `input`, and what to do now. An input that makes no sense in the current
/// state (a stale reply after the editor closed) changes nothing.
pub fn edit_step(state: Edit, input: EditIn) -> (Edit, Vec<EditOut>) {
    use Edit as S;
    use EditIn as I;
    match (state, input) {
        (S::Closed, _) => (S::Closed, vec![]),
        (_, I::Closed) => (S::Closed, vec![EditOut::StopWatching]),

        (S::Opening, I::Fetched { stat }) => (S::Open { base: stat }, vec![EditOut::OpenLocal]),
        (S::Opening, I::FetchGone) => (
            S::Broken(EditFault::Gone),
            vec![EditOut::Report(EditFault::Gone)],
        ),
        (S::Opening, I::Failed(fault)) => (S::Broken(fault), vec![EditOut::Report(fault)]),

        (S::Open { base } | S::Stalled { base }, I::LocalSaved | I::Retry) => (
            S::Checking {
                base,
                queued: Queued::Nothing,
            },
            vec![EditOut::Check],
        ),
        (S::Checking { base, .. }, I::LocalSaved) => (
            S::Checking {
                base,
                queued: Queued::Resave,
            },
            vec![],
        ),
        (S::Uploading { base, .. }, I::LocalSaved) => (
            S::Uploading {
                base,
                queued: Queued::Resave,
            },
            vec![],
        ),

        (S::Checking { base, queued }, I::Checked(RemoteState::Present(now))) if now == base => {
            (S::Uploading { base, queued }, vec![EditOut::Store])
        }
        (S::Checking { base, .. }, I::Checked(RemoteState::Present(now))) => {
            conflict(base, Found::Changed(now))
        }
        (S::Checking { base, .. }, I::Checked(_)) => conflict(base, Found::Deleted),

        (
            S::Uploading {
                queued: Queued::Resave,
                ..
            },
            I::Stored { stat },
        ) => (
            S::Checking {
                base: stat,
                queued: Queued::Nothing,
            },
            vec![EditOut::Saved, EditOut::Check],
        ),
        (S::Uploading { .. }, I::Stored { stat }) => (S::Open { base: stat }, vec![EditOut::Saved]),

        (S::Uploading { base, .. }, I::Conflicted(found)) => conflict(base, found),

        (S::Checking { base, .. } | S::Uploading { base, .. }, I::Failed(fault)) => {
            (S::Stalled { base }, vec![EditOut::Report(fault)])
        }

        (S::Conflict { base, .. }, I::Resolved(Resolution::Overwrite)) => (
            S::Uploading {
                base,
                queued: Queued::Nothing,
            },
            vec![EditOut::Store],
        ),
        (S::Conflict { .. }, I::Resolved(Resolution::TakeRemote)) => {
            (S::Opening, vec![EditOut::Fetch])
        }
        (S::Conflict { base, .. }, I::Resolved(Resolution::Later)) => (S::Open { base }, vec![]),

        (state, _) => (state, vec![]),
    }
}

fn conflict(base: temor_ssh::RemoteStat, found: Found) -> (Edit, Vec<EditOut>) {
    (
        Edit::Conflict { base, found },
        vec![EditOut::AskConflict(found)],
    )
}
