//! The notice rules: focus, the threshold, outcomes, and the limiter on a fake clock.

use ds_core::time::stamp::Stamp;

use super::{Focus, Limiter, Notice, bell_notice, finished_notice, program_notice};
use crate::block::{Author, Block, Outcome};
use crate::vocab::ids::{BlockId, ExitCode, GridRow, GridSpan};
use crate::vocab::text::CommandText;
use temor_pane::settings::NotifyAfter;

fn block(ran_ms: u64, exit: Option<i32>) -> Block {
    let span = GridSpan::row(GridRow(1));
    Block {
        id: BlockId(1),
        prompt: span,
        command: CommandText("make all".into()),
        output: span,
        exit: exit.map(ExitCode),
        by: Author::Person,
        started: Stamp(1_000),
        ended: Some(Stamp(1_000 + ran_ms)),
    }
}

#[test]
fn a_long_command_notifies_only_while_the_pane_is_unfocused() {
    let long = block(12_000, Some(0));
    assert_eq!(
        finished_notice(&long, NotifyAfter::DEFAULT, Focus::Unfocused),
        Some(Notice::CommandFinished {
            outcome: Outcome::Succeeded,
            took_ms: 12_000,
            command: "make all".into(),
        })
    );
    assert_eq!(
        finished_notice(&long, NotifyAfter::DEFAULT, Focus::Focused),
        None
    );
}

#[test]
fn the_threshold_is_inclusive_and_short_commands_stay_quiet() {
    let at = block(10_000, Some(2));
    assert!(matches!(
        finished_notice(&at, NotifyAfter::Seconds(10), Focus::Unfocused),
        Some(Notice::CommandFinished {
            outcome: Outcome::Failed(ExitCode(2)),
            ..
        })
    ));
    assert_eq!(
        finished_notice(
            &block(9_999, Some(0)),
            NotifyAfter::Seconds(10),
            Focus::Unfocused
        ),
        None
    );
}

#[test]
fn off_never_notifies_and_a_running_block_is_not_finished() {
    assert_eq!(
        finished_notice(&block(99_000, Some(0)), NotifyAfter::Off, Focus::Unfocused),
        None
    );
    let mut running = block(0, None);
    running.ended = None;
    assert_eq!(
        finished_notice(&running, NotifyAfter::Seconds(0), Focus::Unfocused),
        None
    );
}

#[test]
fn an_unknown_status_still_notifies_as_unreported() {
    let note = finished_notice(&block(15_000, None), NotifyAfter::DEFAULT, Focus::Unfocused);
    assert!(matches!(
        note,
        Some(Notice::CommandFinished {
            outcome: Outcome::Unreported,
            ..
        })
    ));
}

#[test]
fn program_notices_and_the_bell_follow_focus() {
    assert_eq!(
        program_notice("Deploy", "Apply?", Focus::Unfocused),
        Some(Notice::Program {
            title: "Deploy".into(),
            body: "Apply?".into()
        })
    );
    assert_eq!(program_notice("", "x", Focus::Focused), None);
    assert_eq!(bell_notice(Focus::Unfocused), Some(Notice::Bell));
    assert_eq!(bell_notice(Focus::Focused), None);
}

#[test]
fn the_limiter_lets_one_notice_through_per_gap_on_a_fake_clock() {
    let mut limit = Limiter::new(2_000);
    assert!(limit.allow(Stamp(0)));
    assert!(!limit.allow(Stamp(1_999)), "inside the gap");
    assert!(limit.allow(Stamp(2_000)), "the gap is inclusive");
    assert!(!limit.allow(Stamp(2_500)));
    assert!(limit.allow(Stamp(10_000)));
}
