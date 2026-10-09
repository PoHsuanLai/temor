//! Marks as the session takes them, driven by hand: the numbering that keeps a missed anchor from
//! pairing with the next mark, the anchors of ignored marks, a payload that tries to submit, and
//! the gutter while a full-screen program runs. The shell is real; the marks are written here.

use temor_core::block::Outcome;
use temor_core::vocab::ids::BlockId;
use temor_pane::PaneEvent;
use temor_vt::{ExitCode, ExitStatus, PromptEvent, PromptKind};

use super::super::support::tools_present;
use super::support::{Live, pump, shell_path, start};

/// Sends one whole mark: its number, its event, and its anchor at the cursor.
fn mark(live: &mut Live, seq: u64, event: PromptEvent) {
    live.session.absorb(PaneEvent::MarkBegin { seq });
    live.session.absorb(PaneEvent::Prompt(event));
    if let Some(anchor) = live.session.pane().with_term(|term| term.anchor_cursor()) {
        live.session.absorb(PaneEvent::Anchor { seq, anchor });
    }
}

fn anchors(live: &Live) -> usize {
    live.session.pane().with_term(|term| term.anchor_count())
}

/// A started session at a prompt the shell marked (so the machine is at the typing state).
fn at_prompt(name: &str) -> Option<Live> {
    if !tools_present() {
        return None;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start(name, &shell);
    // The prompt's own mark and the input mark after it: both anchors are held before a test
    // counts anchors, so a late input mark cannot land in the middle of the count.
    pump(&mut live, "the input mark", |s| {
        s.pane().with_term(|term| term.anchor_count()) >= 2
    });
    Some(live)
}

#[test]
fn a_missed_anchor_does_not_pair_with_the_next_marks_event() {
    let Some(mut live) = at_prompt("blocks-pair") else {
        return;
    };
    let prompts = live.session.prompts_seen();
    let before = anchors(&live);
    // The first mark's anchor never arrives.
    live.session.absorb(PaneEvent::MarkBegin { seq: 1000 });
    live.session
        .absorb(PaneEvent::Prompt(PromptEvent::PromptStart(
            PromptKind::Primary,
        )));
    // The next mark is whole. Its anchor must not be paired with the missed prompt.
    mark(&mut live, 1001, PromptEvent::InputStart);
    assert_eq!(
        live.session.prompts_seen(),
        prompts,
        "the missed prompt was not taken"
    );
    assert_eq!(anchors(&live), before, "nothing is left behind");
}

#[test]
fn ignored_marks_keep_no_anchors() {
    let Some(mut live) = at_prompt("blocks-ignored") else {
        return;
    };
    let before = anchors(&live);
    // The shell is typing already: a second input start changes nothing.
    mark(&mut live, 2000, PromptEvent::InputStart);
    assert_eq!(
        anchors(&live),
        before,
        "an ignored input start keeps no anchor"
    );
    // An end with no command running makes no block, so its anchor goes back too.
    mark(
        &mut live,
        2001,
        PromptEvent::CommandEnd {
            exit: ExitStatus::Code(ExitCode(0)),
            error: String::new(),
        },
    );
    assert_eq!(
        anchors(&live),
        before,
        "an end with no command keeps no anchor"
    );
    assert!(live.session.block_outcomes().is_empty());
}

/// A command the shell reports with a payload: the command text and its exit.
fn command_block(live: &mut Live, seq: u64, command: &str) {
    mark(
        live,
        seq,
        PromptEvent::OutputStart {
            command: command.to_owned(),
        },
    );
    mark(
        live,
        seq + 1,
        PromptEvent::CommandEnd {
            exit: ExitStatus::Code(ExitCode(0)),
            error: String::new(),
        },
    );
    mark(live, seq + 2, PromptEvent::PromptStart(PromptKind::Primary));
    mark(live, seq + 3, PromptEvent::InputStart);
}

#[test]
fn a_command_payload_with_a_line_break_is_refused_on_rerun_and_others_are_cleaned() {
    let Some(mut live) = at_prompt("blocks-payload") else {
        return;
    };
    // The person's typed text, as the terminal holds it.
    live.session.pane().with_term(|term| term.write(b"ls"));
    // The payload carries a line break: it would submit `rm` if typed as a line.
    command_block(&mut live, 3000, "ls\nrm -rf /tmp/never");
    // The payload carries an end-of-file control: it is dropped, and the rest is safe to type.
    command_block(&mut live, 3010, "\u{4}echo typed-not-run");
    let ids: Vec<BlockId> = live
        .session
        .block_outcomes()
        .into_iter()
        .map(|b| b.0)
        .collect();
    assert_eq!(ids.len(), 2, "both commands were accepted");

    assert_eq!(
        live.session.block_command(ids[0]),
        Some(""),
        "a line break refuses the text"
    );
    live.session.rerun(ids[0]);
    assert_eq!(
        live.session.take_notice().as_deref(),
        Some("This command cannot be typed again.")
    );

    assert_eq!(
        live.session.block_command(ids[1]),
        Some("echo typed-not-run")
    );
    live.session.rerun(ids[1]);
    assert_eq!(live.session.take_notice(), None, "a safe command is typed");
    // The shell echoes what reached it, and nothing presses Enter: the command never runs.
    pump(&mut live, "the typed command", |s| {
        s.screen_text().contains("echo typed-not-run")
    });
    let settle = std::time::Instant::now();
    while settle.elapsed() < std::time::Duration::from_millis(500) {
        pump(&mut live, "events", |_| true);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let screen = live.session.screen_text();
    assert!(
        !screen.lines().any(|line| line.trim() == "typed-not-run"),
        "the command was submitted:\n{screen}"
    );
    assert_eq!(
        live.session.block_outcomes().len(),
        2,
        "no new block: nothing ran"
    );
    assert_eq!(
        live.session.block_outcomes()[1].1,
        Outcome::Succeeded,
        "its exit was taken from the mark"
    );
}

#[test]
fn a_full_screen_program_hides_the_gutter_and_its_hits() {
    let Some(mut live) = at_prompt("blocks-alt") else {
        return;
    };
    command_block(&mut live, 4000, "vim");
    let id = live.session.block_outcomes()[0].0;
    assert!(
        !live.session.block_marks().is_empty(),
        "the prompt is on screen"
    );
    live.session
        .pane()
        .with_term(|term| term.write(b"\x1b[?1049h"));
    assert!(
        live.session.block_marks().is_empty(),
        "no marks on the alternate screen"
    );
    assert!(
        live.session.block_summary(id).is_some(),
        "the summary still reads while hidden"
    );
    live.session
        .pane()
        .with_term(|term| term.write(b"\x1b[?1049l"));
    assert!(
        !live.session.block_marks().is_empty(),
        "the marks return with the shell's screen"
    );
}
