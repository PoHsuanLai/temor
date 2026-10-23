//! Snapshot round trip: `shell.bin` and `sgr.bin` plus scrollback, encoded and restored into a
//! fresh terminal with the same text, styles, links, cursor, title and modes.

use std::sync::mpsc;

use temor_vt::{
    Formatter, GridSize, ModeState, Snapshot, Terminal, TerminalConfig, TerminalMode, TextFormat,
    TextSpan, VtError, VtEvent,
};

use crate::support::{fixture, term};

fn export(t: &Terminal, format: TextFormat) -> String {
    Formatter::new(format)
        .format(t, TextSpan::Screen)
        .expect("export")
}

#[test]
fn snapshot_restores_text_styles_cursor_title_and_modes() {
    let (mut t, _rx) = term(60, 8);
    t.write(&fixture("shell.bin"));
    t.write(&fixture("sgr.bin"));
    for i in 0..40 {
        t.write(format!("history {i}\r\n").as_bytes());
    }
    t.write(b"\x1b[?2004h\x1b[3;7Hcursor");
    let snap = t.snapshot().expect("encode");
    let stored = Snapshot::from_bytes(snap.as_bytes().to_vec());

    let (tx, rx) = mpsc::channel::<VtEvent>();
    let back = Terminal::restore(
        &stored,
        TerminalConfig::new(GridSize::new(1, 1)),
        Box::new(tx),
        Box::new(mpsc::channel::<Vec<u8>>().0),
    )
    .expect("restore");
    assert_eq!(back.size(), GridSize::new(60, 8));
    assert_eq!(
        export(&back, TextFormat::Plain),
        export(&t, TextFormat::Plain)
    );
    assert_eq!(
        export(&back, TextFormat::Vt),
        export(&t, TextFormat::Vt),
        "styles and links survive"
    );
    assert_eq!(back.title(), "my title");
    assert_eq!(back.mode(TerminalMode::BracketedPaste), Ok(ModeState::Set));
    assert_eq!(back.mode(TerminalMode::GraphemeCluster), Ok(ModeState::Set));
    assert_eq!(back.scrollbar(), t.scrollbar());

    // The restored terminal is live: its callbacks reach the new sink.
    let mut back = back;
    back.write(b"\x07");
    assert_eq!(rx.try_iter().collect::<Vec<_>>(), [VtEvent::Bell]);
}

#[test]
fn corrupt_snapshot_is_rejected() {
    let (tx, _rx) = mpsc::channel::<VtEvent>();
    let junk = Snapshot::from_bytes(b"GHOSTSNPnot really".to_vec());
    let got = Terminal::restore(
        &junk,
        TerminalConfig::new(GridSize::new(10, 4)),
        Box::new(tx),
        Box::new(mpsc::channel::<Vec<u8>>().0),
    );
    assert_eq!(got.err(), Some(VtError::SnapshotRejected));
}

fn restored(snap: &Snapshot) -> Terminal {
    Terminal::restore(
        &Snapshot::from_bytes(snap.as_bytes().to_vec()),
        TerminalConfig::new(GridSize::new(1, 1)),
        Box::new(mpsc::channel::<VtEvent>().0),
        Box::new(mpsc::channel::<Vec<u8>>().0),
    )
    .expect("restore")
}

#[test]
fn a_restored_screens_open_prompt_comes_off_but_a_running_commands_screen_stays() {
    let (mut t, _rx) = term(40, 10);
    // A finished command, then an idle prompt: a blank line, the folder line, the typing line.
    t.write(b"\x1b]133;A\x07\r\n~/work\r\n\x1b]133;B\x07$ ls\r\n\x1b]133;C\x07out\r\n");
    t.write(b"\x1b]133;D;0\x07\x1b]133;A\x07\r\n~/work\r\n\x1b]133;B\x07");
    let text = |t: &Terminal| export(t, TextFormat::Plain);
    let snap = t.snapshot().expect("encode");
    // The snapshot keeps both prompts; the restore takes the open one off, its blank line too.
    let mut back = restored(&snap);
    assert_eq!(text(&back).matches("~/work").count(), 2);
    back.take_open_prompt_off();
    let kept = text(&back);
    assert_eq!(kept.matches("~/work").count(), 1, "{kept:?}");
    assert!(kept.trim_end().ends_with("out"), "{kept:?}");
    // With a command running there is no prompt to take off.
    t.write(b"$ sleep\r\n\x1b]133;C\x07running\r\n");
    let mut back = restored(&t.snapshot().expect("encode"));
    let before = text(&back);
    back.take_open_prompt_off();
    assert_eq!(text(&back), before);
}

#[test]
fn a_one_line_prompt_comes_off_and_the_output_above_it_stays() {
    let (mut t, _rx) = term(40, 10);
    // A one-line prompt (bash's default): the mark is on the cursor's own row.
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07first\r\nlast\r\n");
    t.write(b"\x1b]133;D;0\x07\x1b]133;A\x07$ \x1b]133;B\x07");
    let mut back = restored(&t.snapshot().expect("encode"));
    back.take_open_prompt_off();
    let kept = export(&back, TextFormat::Plain);
    assert!(
        kept.contains("last"),
        "the output's last line stays: {kept:?}"
    );
    assert_eq!(
        kept.matches("$ ").count(),
        1,
        "only the idle prompt comes off: {kept:?}"
    );
}

/// A restored screen keeps which text was prompt, typed command and output, so its blocks can be
/// found again although its marks never arrive as events.
#[test]
fn a_restored_screen_says_where_prompts_commands_and_output_start() {
    let (mut t, _rx) = term(40, 6);
    for n in 0..2 {
        t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07");
        t.write(format!("echo {n}\r\n\x1b]133;C\x07{n}\r\n\x1b]133;D;0\x07").as_bytes());
    }
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07");
    let stored = Snapshot::from_bytes(t.snapshot().expect("encode").as_bytes().to_vec());
    let back = Terminal::restore(
        &stored,
        TerminalConfig::new(GridSize::new(1, 1)),
        Box::new(mpsc::channel::<VtEvent>().0),
        Box::new(mpsc::channel::<Vec<u8>>().0),
    )
    .expect("restore");
    let starts: Vec<_> = back
        .semantic_starts()
        .expect("starts")
        .into_iter()
        .map(|(at, kind)| (at.row, at.col.0, kind))
        .collect();
    use temor_vt::SemanticMark::{Input, Output, Prompt};
    assert_eq!(
        starts,
        [
            (0, 0, Prompt),
            (0, 2, Input),
            (1, 0, Output),
            (2, 0, Prompt),
            (2, 2, Input),
            (3, 0, Output),
            (4, 0, Prompt),
        ],
        "{starts:?}"
    );
    assert_eq!(back.semantic_starts(), t.semantic_starts());
}
