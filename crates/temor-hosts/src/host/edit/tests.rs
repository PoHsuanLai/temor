use super::{
    Edit, EditFault, EditIn, EditOut, Found, LocalStamp, Resolution, Watch, edit_step, watch_step,
};
use temor_ssh::{ByteCount, RemoteStat, RemoteState, UnixSeconds};
use temor_testkit::run_steps;

fn stat(mtime: u64, size: u64) -> RemoteStat {
    RemoteStat {
        mtime: UnixSeconds(mtime),
        size: ByteCount(size),
    }
}

const V1: RemoteStat = RemoteStat {
    mtime: UnixSeconds(100),
    size: ByteCount(10),
};

/// Runs `inputs` from `from`, returning the state and every output in order.
fn run(from: Edit, inputs: &[EditIn]) -> (Edit, Vec<EditOut>) {
    run_steps(from, inputs.iter().copied(), edit_step)
}

#[test]
fn a_save_is_checked_against_the_host_then_written_back() {
    let v2 = stat(200, 12);
    let (state, outs) = run(
        Edit::Opening,
        &[
            EditIn::Fetched { stat: V1 },
            EditIn::LocalSaved,
            EditIn::Checked(RemoteState::Present(V1)),
            EditIn::Stored { stat: v2 },
        ],
    );
    assert_eq!(state, Edit::Open { base: v2 });
    assert_eq!(
        outs,
        vec![
            EditOut::OpenLocal,
            EditOut::Check,
            EditOut::Store,
            EditOut::Saved
        ]
    );
}

#[test]
fn saves_made_during_a_write_back_are_written_in_turn() {
    let v2 = stat(200, 12);
    let (state, outs) = run(
        Edit::Open { base: V1 },
        &[
            EditIn::LocalSaved,
            EditIn::LocalSaved,
            EditIn::Checked(RemoteState::Present(V1)),
            EditIn::LocalSaved,
            EditIn::Stored { stat: v2 },
        ],
    );
    assert_eq!(
        state,
        Edit::Checking {
            base: v2,
            queued: false
        }
    );
    assert_eq!(
        outs,
        vec![
            EditOut::Check,
            EditOut::Store,
            EditOut::Saved,
            EditOut::Check
        ]
    );
}

#[test]
fn a_changed_or_removed_host_file_is_never_overwritten_without_asking() {
    // Name, what the host says, the conflict.
    let changed = stat(150, 10);
    let cases = [
        (
            "edited there",
            RemoteState::Present(changed),
            Found::Changed(changed),
        ),
        (
            "same time, new size",
            RemoteState::Present(stat(100, 11)),
            Found::Changed(stat(100, 11)),
        ),
        ("removed there", RemoteState::Missing, Found::Deleted),
    ];
    for (name, host_says, found) in cases {
        let (state, outs) = run(
            Edit::Open { base: V1 },
            &[EditIn::LocalSaved, EditIn::Checked(host_says)],
        );
        assert_eq!(state, Edit::Conflict { base: V1, found }, "{name}");
        assert_eq!(
            outs,
            vec![EditOut::Check, EditOut::AskConflict(found)],
            "{name}"
        );
    }
}

#[test]
fn a_conflict_waits_for_the_person_and_each_choice_does_what_it_says() {
    let found = Found::Changed(stat(150, 10));
    let at = Edit::Conflict { base: V1, found };
    // Name, choice, state after, outputs.
    let cases = [
        (
            "overwrite",
            Resolution::Overwrite,
            Edit::Uploading {
                base: V1,
                queued: false,
            },
            vec![EditOut::Store],
        ),
        (
            "take the host's",
            Resolution::TakeRemote,
            Edit::Opening,
            vec![EditOut::Fetch],
        ),
        ("later", Resolution::Later, Edit::Open { base: V1 }, vec![]),
    ];
    for (name, choice, want, outs) in cases {
        assert_eq!(
            edit_step(at, EditIn::Resolved(choice)),
            (want, outs),
            "{name}"
        );
    }
    assert_eq!(
        edit_step(at, EditIn::LocalSaved),
        (at, vec![]),
        "saving again does not skip the question"
    );
}

#[test]
fn a_write_the_host_refuses_as_changed_asks_the_person_and_never_lands() {
    let changed = Found::Changed(stat(150, 10));
    // Name, state when the refusal arrives, input, state after, outputs.
    let uploading = Edit::Uploading {
        base: V1,
        queued: true,
    };
    let cases = [
        (
            "changed since the check",
            uploading,
            EditIn::Conflicted(changed),
            Edit::Conflict {
                base: V1,
                found: changed,
            },
            vec![EditOut::AskConflict(changed)],
        ),
        (
            "removed since the check",
            uploading,
            EditIn::Conflicted(Found::Deleted),
            Edit::Conflict {
                base: V1,
                found: Found::Deleted,
            },
            vec![EditOut::AskConflict(Found::Deleted)],
        ),
        (
            "a late refusal after closing",
            Edit::Closed,
            EditIn::Conflicted(changed),
            Edit::Closed,
            vec![],
        ),
        (
            "a refusal with nothing being written",
            Edit::Open { base: V1 },
            EditIn::Conflicted(changed),
            Edit::Open { base: V1 },
            vec![],
        ),
    ];
    for (name, from, input, want, outs) in cases {
        assert_eq!(edit_step(from, input), (want, outs), "{name}");
    }
}

#[test]
fn a_failed_call_stalls_the_edit_and_a_save_or_retry_starts_over() {
    for (name, failing) in [
        (
            "checking",
            Edit::Checking {
                base: V1,
                queued: false,
            },
        ),
        (
            "uploading",
            Edit::Uploading {
                base: V1,
                queued: true,
            },
        ),
    ] {
        let (stalled, outs) = edit_step(failing, EditIn::Failed(EditFault::NoConnection));
        assert_eq!(stalled, Edit::Stalled { base: V1 }, "{name}");
        assert_eq!(
            outs,
            vec![EditOut::Report(EditFault::NoConnection)],
            "{name}"
        );
        for again in [EditIn::LocalSaved, EditIn::Retry] {
            assert_eq!(
                edit_step(stalled, again),
                (
                    Edit::Checking {
                        base: V1,
                        queued: false
                    },
                    vec![EditOut::Check]
                ),
                "{name}"
            );
        }
    }
    let (broken, outs) = edit_step(Edit::Opening, EditIn::FetchGone);
    assert_eq!(broken, Edit::Broken(EditFault::Gone));
    assert_eq!(outs, vec![EditOut::Report(EditFault::Gone)]);
}

#[test]
fn closing_stops_the_watch_and_late_replies_are_ignored() {
    let (closed, outs) = edit_step(
        Edit::Uploading {
            base: V1,
            queued: false,
        },
        EditIn::Closed,
    );
    assert_eq!((closed, outs), (Edit::Closed, vec![EditOut::StopWatching]));
    assert_eq!(
        edit_step(closed, EditIn::Stored { stat: stat(1, 1) }),
        (Edit::Closed, vec![])
    );
}

fn stamp(mtime: u64, size: u64) -> LocalStamp {
    LocalStamp {
        mtime_nanos: mtime,
        size: ByteCount(size),
    }
}

#[test]
fn a_change_counts_as_a_save_once_it_holds_still_for_two_polls() {
    // Polls in order, and after each the save reported (if any).
    let polls = [
        (Some(stamp(1, 5)), false, "unchanged"),
        (Some(stamp(2, 0)), false, "truncated first"),
        (None, false, "the editor's rename gap"),
        (Some(stamp(3, 9)), false, "written, still moving"),
        (Some(stamp(3, 9)), true, "held still: a save"),
        (Some(stamp(3, 9)), false, "nothing new"),
        (Some(stamp(4, 9)), false, "edited again"),
        (
            Some(stamp(3, 9)),
            false,
            "put back before it settled: no save",
        ),
    ];
    let mut watch = Watch::new(stamp(1, 5));
    for (polled, saved, name) in polls {
        let (next, out) = watch_step(watch, polled);
        assert_eq!(out == Some(EditIn::LocalSaved), saved, "{name}");
        watch = next;
    }
    assert_eq!(
        watch,
        Watch::Idle {
            settled: stamp(3, 9)
        }
    );
}
