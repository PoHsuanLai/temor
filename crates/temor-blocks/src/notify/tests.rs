//! The notice rules: focus, the threshold, outcomes, and the limiter on a fake clock.

use ds_core::time::stamp::Stamp;

use super::{Limiter, Notice, PaneFocus, bell_notice, finished_notice, program_notice};
use crate::block::{Author, Block, Outcome};
use temor_core::vocab::ids::{BlockId, ExitCode, GridRow, GridSpan};
use temor_core::vocab::text::CommandText;
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
        finished_notice(&long, NotifyAfter::DEFAULT, PaneFocus::Unfocused),
        Some(Notice::CommandFinished {
            outcome: Outcome::Succeeded,
            took_ms: 12_000,
            command: "make all".into(),
        })
    );
    assert_eq!(
        finished_notice(&long, NotifyAfter::DEFAULT, PaneFocus::Focused),
        None
    );
}

#[test]
fn the_threshold_is_inclusive_and_short_commands_stay_quiet() {
    let at = block(10_000, Some(2));
    assert!(matches!(
        finished_notice(&at, NotifyAfter::Seconds(10), PaneFocus::Unfocused),
        Some(Notice::CommandFinished {
            outcome: Outcome::Failed(ExitCode(2)),
            ..
        })
    ));
    assert_eq!(
        finished_notice(
            &block(9_999, Some(0)),
            NotifyAfter::Seconds(10),
            PaneFocus::Unfocused
        ),
        None
    );
}

#[test]
fn a_stopped_command_notifies_as_finished_not_as_failed() {
    let note = finished_notice(
        &block(15_000, Some(130)),
        NotifyAfter::DEFAULT,
        PaneFocus::Unfocused,
    );
    assert!(matches!(
        note,
        Some(Notice::CommandFinished {
            outcome: Outcome::Stopped(ExitCode(130)),
            ..
        })
    ));
}

#[test]
fn off_never_notifies_and_a_running_block_is_not_finished() {
    assert_eq!(
        finished_notice(
            &block(99_000, Some(0)),
            NotifyAfter::Off,
            PaneFocus::Unfocused
        ),
        None
    );
    let mut running = block(0, None);
    running.ended = None;
    assert_eq!(
        finished_notice(&running, NotifyAfter::Seconds(0), PaneFocus::Unfocused),
        None
    );
}

#[test]
fn an_unknown_status_still_notifies_as_unreported() {
    let note = finished_notice(
        &block(15_000, None),
        NotifyAfter::DEFAULT,
        PaneFocus::Unfocused,
    );
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
        program_notice("Deploy", "Apply?", PaneFocus::Unfocused),
        Some(Notice::Program {
            title: "Deploy".into(),
            body: "Apply?".into()
        })
    );
    assert_eq!(program_notice("", "x", PaneFocus::Focused), None);
    assert_eq!(bell_notice(PaneFocus::Unfocused), Some(Notice::Bell));
    assert_eq!(bell_notice(PaneFocus::Focused), None);
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
