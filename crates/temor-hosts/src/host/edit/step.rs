//! `edit_step`: one remote edit from the fetch to the write-back, and to its close.

use super::model::{Edit, EditFault, EditIn, EditOut, Found, Resolution};
use temor_ssh::{RemoteStat, RemoteState};

/// The edit after `input`, and what to do now. An input that makes no sense in the current
/// state (a stale reply after the editor closed) changes nothing.
pub fn edit_step(state: Edit, input: EditIn) -> (Edit, Vec<EditOut>) {
    use Edit as S;
    use EditIn as I;
    match (state, input) {
        (S::Closed, _) => (S::Closed, vec![]),

        // Done while a choice is open keeps the copy: it holds the only copy of the edits.
        (S::Conflict { .. }, I::Done) => (state, vec![EditOut::ChooseFirst]),
        // Done while a save is on its way: the edit closes once the save is on the host, as an
        // editor finishes saving before it closes a file.
        (S::Checking { base, queued, .. }, I::Done) => (
            S::Checking {
                base,
                queued,
                closing: true,
            },
            vec![],
        ),
        (S::Uploading { base, queued, .. }, I::Done) => (
            S::Uploading {
                base,
                queued,
                closing: true,
            },
            vec![],
        ),
        (_, I::Done) => (S::Closed, closed()),

        (S::Opening { shown }, I::Fetched { stat }) => (
            S::Open { base: stat },
            match shown {
                true => vec![],
                false => vec![EditOut::OpenLocal],
            },
        ),
        (S::Opening { .. }, I::FetchGone) => (
            S::Broken(EditFault::Gone),
            vec![EditOut::Report(EditFault::Gone)],
        ),
        (S::Opening { .. }, I::Failed(fault)) => (S::Broken(fault), vec![EditOut::Report(fault)]),

        (S::Open { base } | S::Stalled { base }, I::LocalSaved | I::Retry) => (
            S::Checking {
                base,
                queued: false,
                closing: false,
            },
            vec![EditOut::Check],
        ),
        (S::Checking { base, closing, .. }, I::LocalSaved) => (
            S::Checking {
                base,
                queued: true,
                closing,
            },
            vec![],
        ),
        (S::Uploading { base, closing, .. }, I::LocalSaved) => (
            S::Uploading {
                base,
                queued: true,
                closing,
            },
            vec![],
        ),

        (
            S::Checking {
                base,
                queued,
                closing,
            },
            I::Checked(RemoteState::Present(now)),
        ) if now == base => (
            S::Uploading {
                base,
                queued,
                closing,
            },
            vec![EditOut::Store { overwrite: false }],
        ),
        (S::Checking { base, closing, .. }, I::Checked(RemoteState::Present(now))) => {
            conflict(base, Found::Changed(now), closing)
        }
        (S::Checking { base, closing, .. }, I::Checked(_)) => {
            conflict(base, Found::Deleted, closing)
        }

        (
            S::Uploading {
                queued: true,
                closing,
                ..
            },
            I::Stored { stat },
        ) => (
            S::Checking {
                base: stat,
                queued: false,
                closing,
            },
            vec![EditOut::Saved, EditOut::Check],
        ),
        (S::Uploading { closing: true, .. }, I::Stored { .. }) => {
            (S::Closed, [vec![EditOut::Saved], closed()].concat())
        }
        (S::Uploading { .. }, I::Stored { stat }) => (S::Open { base: stat }, vec![EditOut::Saved]),

        (S::Uploading { base, closing, .. }, I::Conflicted(found)) => {
            conflict(base, found, closing)
        }

        (
            S::Checking { base, closing, .. } | S::Uploading { base, closing, .. },
            I::Failed(fault),
        ) => (
            S::Stalled { base },
            kept(vec![EditOut::Report(fault)], closing),
        ),

        (S::Conflict { base, .. }, I::Resolved(Resolution::Overwrite)) => (
            S::Uploading {
                base,
                queued: false,
                closing: false,
            },
            vec![EditOut::Store { overwrite: true }],
        ),
        // The copy is already open: the host's file replaces it in place.
        (S::Conflict { .. }, I::Resolved(Resolution::TakeRemote)) => {
            (S::Opening { shown: true }, vec![EditOut::Fetch])
        }
        (S::Conflict { base, .. }, I::Resolved(Resolution::Later)) => (S::Open { base }, vec![]),

        (state, _) => (state, vec![]),
    }
}

/// What closing does: the watch stops and the edit and its copy go.
fn closed() -> Vec<EditOut> {
    vec![EditOut::StopWatching, EditOut::Discard]
}

/// `outs`, and when Done was waiting on a save that did not get there, that the copy is kept
/// (Done has to be pressed again).
fn kept(mut outs: Vec<EditOut>, closing: bool) -> Vec<EditOut> {
    if closing {
        outs.push(EditOut::Kept);
    }
    outs
}

fn conflict(base: RemoteStat, found: Found, closing: bool) -> (Edit, Vec<EditOut>) {
    (
        Edit::Conflict { base, found },
        kept(vec![EditOut::AskConflict(found)], closing),
    )
}
