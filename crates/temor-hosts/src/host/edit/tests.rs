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
        Edit::Opening { shown: false },
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
            EditOut::Store { overwrite: false },
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
            queued: false,
            closing: false,
        }
    );
    assert_eq!(
        outs,
        vec![
            EditOut::Check,
            EditOut::Store { overwrite: false },
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
                closing: false,
            },
            vec![EditOut::Store { overwrite: true }],
        ),
        (
            "take the host's",
            Resolution::TakeRemote,
            Edit::Opening { shown: true },
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
        closing: false,
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
                closing: false,
            },
        ),
        (
            "uploading",
            Edit::Uploading {
                base: V1,
                queued: true,
                closing: false,
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
                        queued: false,
                        closing: false,
                    },
                    vec![EditOut::Check]
                ),
                "{name}"
            );
        }
    }
    let (broken, outs) = edit_step(Edit::Opening { shown: false }, EditIn::FetchGone);
    assert_eq!(broken, Edit::Broken(EditFault::Gone));
    assert_eq!(outs, vec![EditOut::Report(EditFault::Gone)]);
}

#[test]
fn done_closes_at_once_waits_for_a_save_on_its_way_and_never_drops_a_conflict() {
    let v2 = stat(200, 12);
    let uploading = |queued| Edit::Uploading {
        base: V1,
        queued,
        closing: false,
    };
    let closed = vec![EditOut::StopWatching, EditOut::Discard];
    // Name, the edit, the inputs from Done on, the state after, every output.
    let cases = [
        (
            "nothing on its way",
            Edit::Open { base: V1 },
            vec![EditIn::Done],
            Edit::Closed,
            closed.clone(),
        ),
        (
            "a save that gets there",
            uploading(false),
            vec![EditIn::Done, EditIn::Stored { stat: v2 }],
            Edit::Closed,
            [vec![EditOut::Saved], closed.clone()].concat(),
        ),
        (
            "a save with another queued: both go first",
            uploading(true),
            vec![
                EditIn::Done,
                EditIn::Stored { stat: v2 },
                EditIn::Checked(RemoteState::Present(v2)),
                EditIn::Stored { stat: V1 },
            ],
            Edit::Closed,
            [
                vec![
                    EditOut::Saved,
                    EditOut::Check,
                    EditOut::Store { overwrite: false },
                    EditOut::Saved,
                ],
                closed.clone(),
            ]
            .concat(),
        ),
        (
            "a save that fails keeps the copy",
            uploading(false),
            vec![EditIn::Done, EditIn::Failed(EditFault::NoConnection)],
            Edit::Stalled { base: V1 },
            vec![EditOut::Report(EditFault::NoConnection), EditOut::Kept],
        ),
        (
            "a save the host refuses keeps the copy and asks",
            uploading(false),
            vec![EditIn::Done, EditIn::Conflicted(Found::Deleted)],
            Edit::Conflict {
                base: V1,
                found: Found::Deleted,
            },
            vec![EditOut::AskConflict(Found::Deleted), EditOut::Kept],
        ),
        (
            "a choice that is open",
            Edit::Conflict {
                base: V1,
                found: Found::Deleted,
            },
            vec![EditIn::Done],
            Edit::Conflict {
                base: V1,
                found: Found::Deleted,
            },
            vec![EditOut::ChooseFirst],
        ),
        (
            "a late reply after closing",
            Edit::Closed,
            vec![EditIn::Stored { stat: v2 }],
            Edit::Closed,
            vec![],
        ),
    ];
    for (name, from, inputs, want, outs) in cases {
        assert_eq!(run(from, &inputs), (want, outs), "{name}");
    }
    // A Done that was waiting is over once the save did not get there: the next save does not
    // close the edit.
    let (stalled, _) = run(
        uploading(false),
        &[EditIn::Done, EditIn::Failed(EditFault::NoConnection)],
    );
    let (state, _) = run(
        stalled,
        &[
            EditIn::Retry,
            EditIn::Checked(RemoteState::Present(V1)),
            EditIn::Stored { stat: v2 },
        ],
    );
    assert_eq!(state, Edit::Open { base: v2 });
}

#[test]
fn the_copy_is_opened_once_and_taking_the_hosts_file_replaces_it_in_place() {
    let changed = Found::Changed(stat(150, 10));
    let (state, outs) = run(
        Edit::Opening { shown: false },
        &[
            EditIn::Fetched { stat: V1 },
            EditIn::LocalSaved,
            EditIn::Checked(RemoteState::Present(stat(150, 10))),
            EditIn::Resolved(Resolution::TakeRemote),
            EditIn::Fetched {
                stat: stat(150, 10),
            },
        ],
    );
    assert_eq!(
        state,
        Edit::Open {
            base: stat(150, 10)
        }
    );
    assert_eq!(
        outs,
        vec![
            EditOut::OpenLocal,
            EditOut::Check,
            EditOut::AskConflict(changed),
            EditOut::Fetch
        ]
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
