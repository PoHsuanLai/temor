//! The mark log's step: marks arriving in their three parts, and what is kept or given back.

use ds_core::time::stamp::Stamp;

use super::{Anchor, BlockAnchors, MarkIn, MarkKind, MarkLog, MarkOut, mark_step};
use crate::block::{BlockState, Outcome};
use crate::test_support::run_steps;
use crate::vocab::ids::{BlockId, ExitCode, GridRow};

/// A whole mark: its number, its event, its anchor (found on `row`).
fn mark(seq: u64, kind: MarkKind, anchor: u64, row: u64) -> Vec<MarkIn> {
    vec![
        MarkIn::Begin { seq },
        MarkIn::Event(kind),
        MarkIn::Anchored {
            seq,
            anchor: Anchor(anchor),
            row: Some(GridRow(row)),
            at: Stamp(row),
            typed: None,
        },
    ]
}

fn prompt(seq: u64, anchor: u64, row: u64) -> Vec<MarkIn> {
    mark(seq, MarkKind::PromptStart { primary: true }, anchor, row)
}

fn input(seq: u64, anchor: u64, row: u64) -> Vec<MarkIn> {
    mark(seq, MarkKind::InputStart, anchor, row)
}

fn output(seq: u64, anchor: u64, row: u64, command: &str) -> Vec<MarkIn> {
    mark(
        seq,
        MarkKind::OutputStart {
            command: command.to_owned(),
        },
        anchor,
        row,
    )
}

fn end(seq: u64, anchor: u64, row: u64, exit: Option<i32>) -> Vec<MarkIn> {
    mark(
        seq,
        MarkKind::CommandEnd {
            exit: exit.map(ExitCode),
        },
        anchor,
        row,
    )
}

fn run(log: MarkLog, inputs: Vec<Vec<MarkIn>>) -> (MarkLog, Vec<MarkOut>) {
    run_steps(log, inputs.into_iter().flatten(), mark_step)
}

fn released(outs: &[MarkOut]) -> Vec<u64> {
    outs.iter()
        .filter_map(|out| match out {
            MarkOut::Release(Anchor(id)) => Some(*id),
            _ => None,
        })
        .collect()
}

fn command_of(log: &MarkLog, id: u64) -> String {
    log.store()
        .get(BlockId(id))
        .map(|block| block.command.0.clone())
        .unwrap_or_default()
}

/// A prompt, its input mark, then a command with the output and end marks: anchors 1 to 4.
fn one_command(command: &str, exit: Option<i32>) -> Vec<Vec<MarkIn>> {
    vec![
        prompt(1, 1, 0),
        input(2, 2, 1),
        output(3, 3, 2, command),
        end(4, 4, 5, exit),
    ]
}

#[test]
fn a_command_makes_a_block_that_keeps_its_four_anchors_and_says_it_finished() {
    let (log, outs) = run(MarkLog::default(), one_command("ls", Some(0)));
    assert_eq!(command_of(&log, 1), "ls");
    assert_eq!(
        log.store().get(BlockId(1)).map(|b| b.outcome()),
        Some(Outcome::Succeeded)
    );
    assert_eq!(
        log.anchors_of(BlockId(1)),
        Some(BlockAnchors {
            prompt: Some(Anchor(1)),
            input: Some(Anchor(2)),
            output: Some(Anchor(3)),
            end: Some(Anchor(4)),
        })
    );
    assert_eq!(released(&outs), Vec::<u64>::new(), "nothing is given back");
    assert!(matches!(outs.as_slice(), [MarkOut::Finished(block)] if block.id == BlockId(1)));
    assert_eq!((log.prompts(), log.finished()), (1, 1));
    assert_eq!(log.next_id(), BlockId(2));
}

#[test]
fn the_command_the_shell_sent_wins_over_what_was_typed() {
    let typed = |text: Option<&str>| MarkIn::Anchored {
        seq: 3,
        anchor: Anchor(3),
        row: Some(GridRow(2)),
        at: Stamp(2),
        typed: text.map(str::to_owned),
    };
    let build = |command: &str, text: Option<&str>| {
        let (log, _) = run(MarkLog::default(), vec![prompt(1, 1, 0), input(2, 2, 1)]);
        let (log, _) = run_steps(
            log,
            [
                MarkIn::Begin { seq: 3 },
                MarkIn::Event(MarkKind::OutputStart {
                    command: command.to_owned(),
                }),
                typed(text),
            ],
            mark_step,
        );
        command_of(&log, 1)
    };
    // (what, command from the shell, text read from the screen, the block's command)
    for (what, sent, read, want) in [
        ("the shell's own text", "ls -l", Some("ls"), "ls -l"),
        (
            "the typed text when the shell sent none",
            "",
            Some("ls"),
            "ls",
        ),
        ("neither", "", None, ""),
    ] {
        assert_eq!(build(sent, read), want, "{what}");
    }
}

#[test]
fn an_anchor_that_cannot_pair_is_given_back() {
    let lone = |seq, pending_seq, row| {
        run_steps(
            MarkLog::default(),
            [
                MarkIn::Begin { seq: pending_seq },
                MarkIn::Event(MarkKind::InputStart),
                MarkIn::Anchored {
                    seq,
                    anchor: Anchor(9),
                    row,
                    at: Stamp(1),
                    typed: None,
                },
            ],
            mark_step,
        )
    };
    // (what, anchor's number, current mark's number, where the edge found it)
    for (what, seq, current, row) in [
        ("another mark's anchor", 2, 1, Some(GridRow(1))),
        ("an anchor the terminal cannot place", 1, 1, None),
    ] {
        let (log, outs) = lone(seq, current, row);
        assert_eq!(released(&outs), [9], "{what}");
        assert_eq!(log.input(), None, "{what}");
    }
    // An anchor with no mark in progress at all.
    let (_, outs) = mark_step(
        MarkLog::default(),
        MarkIn::Anchored {
            seq: 1,
            anchor: Anchor(5),
            row: Some(GridRow(0)),
            at: Stamp(0),
            typed: None,
        },
    );
    assert_eq!(released(&outs), [5], "an anchor with no mark");
}

#[test]
fn a_mark_the_machine_ignores_leaves_no_anchor_behind() {
    // An input mark and a command start with no prompt before them: the machine stays idle.
    let (log, outs) = run(
        MarkLog::default(),
        vec![input(1, 1, 0), output(2, 2, 1, "ls")],
    );
    assert_eq!(released(&outs), [1, 2]);
    assert_eq!(log.store().len(), 0);
    assert_eq!(log.next_id(), BlockId(1), "no id was spent");
    // A secondary prompt and an unknown mark are given back as well.
    let (_, outs) = run(
        MarkLog::default(),
        vec![
            mark(1, MarkKind::PromptStart { primary: false }, 7, 0),
            mark(2, MarkKind::Other, 8, 0),
        ],
    );
    assert_eq!(released(&outs), [7, 8]);
}

#[test]
fn a_new_prompt_gives_back_the_prompt_and_input_anchors_it_replaces() {
    let (log, outs) = run(
        MarkLog::default(),
        vec![prompt(1, 1, 0), input(2, 2, 1), prompt(3, 3, 4)],
    );
    assert_eq!(released(&outs), [1, 2]);
    assert_eq!((log.prompt(), log.input()), (Some(Anchor(3)), None));
    assert_eq!(log.prompts(), 2);
}

#[test]
fn the_enter_anchor_is_replaced_and_given_back_at_the_next_prompt_or_command() {
    let (log, outs) = run(
        MarkLog::default(),
        vec![
            prompt(1, 1, 0),
            input(2, 2, 1),
            vec![MarkIn::Entered {
                anchor: Some(Anchor(10)),
            }],
            vec![MarkIn::Entered {
                anchor: Some(Anchor(11)),
            }],
        ],
    );
    assert_eq!(
        released(&outs),
        [10],
        "the first Enter's anchor is replaced"
    );
    assert_eq!(log.enter(), Some(Anchor(11)));
    let (log, outs) = run(log, vec![output(3, 3, 2, "ls")]);
    assert_eq!(
        released(&outs),
        [11],
        "the command takes Enter's anchor back"
    );
    assert_eq!(log.enter(), None);
}

/// Two commands: anchors 1 to 4 for `ls`, 5 to 8 for `pwd`.
fn two_commands() -> Vec<Vec<MarkIn>> {
    let mut inputs = one_command("ls", Some(0));
    inputs.extend([
        prompt(5, 5, 6),
        input(6, 6, 7),
        output(7, 7, 8, "pwd"),
        end(8, 8, 9, Some(0)),
    ]);
    inputs
}

#[test]
fn blocks_the_scrollback_dropped_leave_with_their_anchors() {
    let (log, _) = run(MarkLog::default(), two_commands());
    assert_eq!(log.store().len(), 2);
    let (log, outs) = mark_step(
        log,
        MarkIn::Gone {
            ids: vec![BlockId(1)],
        },
    );
    assert_eq!(released(&outs), [1, 2, 3, 4]);
    assert!(outs.contains(&MarkOut::Dropped(vec![BlockId(1)])));
    assert_eq!(log.anchors_of(BlockId(1)), None);
    assert_eq!(log.store().len(), 1);
    assert_eq!(command_of(&log, 2), "pwd", "the other block stays");
}

#[test]
fn the_shell_going_ends_a_running_command_unreported() {
    let (log, _) = run(
        MarkLog::default(),
        vec![prompt(1, 1, 0), input(2, 2, 1), output(3, 3, 2, "sleep 9")],
    );
    assert!(matches!(log.machine(), BlockState::Running(_)));
    let (log, outs) = mark_step(log, MarkIn::ShellGone { at: Stamp(50) });
    assert!(matches!(log.machine(), BlockState::Exited));
    assert!(matches!(outs.as_slice(), [MarkOut::Finished(block)] if block.exit.is_none()));
    assert_eq!(log.finished(), 1);
}

#[test]
fn a_reset_ends_a_running_command_and_a_stray_end_mark_is_given_back() {
    let (log, _) = run(
        MarkLog::default(),
        vec![prompt(1, 1, 0), input(2, 2, 1), output(3, 3, 2, "top")],
    );
    let (log, outs) = mark_step(log, MarkIn::Reset { at: Stamp(60) });
    assert!(matches!(log.machine(), BlockState::Idle));
    assert!(matches!(outs.as_slice(), [MarkOut::Finished(_)]));
    // An end mark with nothing running is not kept.
    let (_, outs) = run(log, vec![end(4, 9, 7, Some(0))]);
    assert_eq!(released(&outs), [9]);
}

#[test]
fn replaying_the_same_marks_in_one_go_or_two_gives_the_same_log() {
    let inputs: Vec<MarkIn> = two_commands().into_iter().flatten().collect();
    let (whole, whole_outs) = run_steps(MarkLog::default(), inputs.clone(), mark_step);
    let (head, tail) = inputs.split_at(inputs.len() / 2);
    let (half, mut outs) = run_steps(MarkLog::default(), head.to_vec(), mark_step);
    let (split, more) = run_steps(half, tail.to_vec(), mark_step);
    outs.extend(more);
    assert_eq!(whole, split);
    assert_eq!(whole_outs, outs);
}

#[test]
fn restored_blocks_join_the_log_finished_with_their_anchors_and_nothing_is_announced() {
    use crate::vocab::ids::GridSpan;
    let span = |a: u64, b: u64| GridSpan {
        first: GridRow(a),
        last: GridRow(b),
    };
    let kept = |n: u64, exit: i32| super::KeptBlock {
        anchors: super::BlockAnchors {
            prompt: Some(super::Anchor(n * 10)),
            input: Some(super::Anchor(n * 10 + 1)),
            output: Some(super::Anchor(n * 10 + 2)),
            end: None,
        },
        prompt: span(n * 3, n * 3),
        output: span(n * 3 + 1, n * 3 + 2),
        command: format!("cmd{n}"),
        exit: Some(ExitCode(exit)),
        took_ms: Some(40),
    };
    let (log, outs) = super::mark_step(
        super::MarkLog::default(),
        super::MarkIn::Restored {
            blocks: vec![kept(1, 0), kept(2, 3)],
            at: Stamp(1000),
        },
    );
    assert!(outs.is_empty(), "{outs:?}");
    let blocks: Vec<_> = log.store().iter().collect();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[1].command.0, "cmd2");
    assert_eq!(blocks[1].exit, Some(ExitCode(3)));
    assert_eq!(blocks[1].elapsed_ms(), Some(40));
    assert_eq!(
        log.anchors_of(blocks[0].id).and_then(|a| a.output),
        Some(super::Anchor(12))
    );
    assert_eq!(log.next_id(), BlockId(3));
}
