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

#[test]
fn the_prompt_being_typed_at_is_left_out_of_a_snapshot_but_a_running_commands_screen_is_kept() {
    let (mut t, _rx) = term(40, 10);
    // A finished command, then an idle prompt: a blank line, the folder line, the typing line.
    t.write(b"\x1b]133;A\x07\r\n~/work\r\n\x1b]133;B\x07$ ls\r\n\x1b]133;C\x07out\r\n");
    t.write(b"\x1b]133;D;0\x07\x1b]133;A\x07\r\n~/work\r\n\x1b]133;B\x07");
    let snap = t.snapshot().expect("encode");
    let text = |t: &Terminal| export(t, TextFormat::Plain);
    let restored = |snap: &Snapshot| {
        Terminal::restore(
            &Snapshot::from_bytes(snap.as_bytes().to_vec()),
            TerminalConfig::new(GridSize::new(1, 1)),
            Box::new(mpsc::channel::<VtEvent>().0),
            Box::new(mpsc::channel::<Vec<u8>>().0),
        )
        .expect("restore")
    };
    // The plain snapshot keeps both prompts.
    assert_eq!(text(&restored(&snap)).matches("~/work").count(), 2);
    t.take_prompt_off();
    let snap = t.snapshot().expect("encode");
    let kept = text(&restored(&snap));
    assert_eq!(kept.matches("~/work").count(), 1, "{kept:?}");
    assert!(kept.contains("out"), "{kept:?}");
    // With a command running there is no prompt to take off.
    t.write(b"$ sleep\r\n\x1b]133;C\x07running\r\n");
    let before = text(&t);
    t.take_prompt_off();
    assert_eq!(text(&t), before);
}

#[test]
fn a_one_line_prompt_comes_off_and_the_output_above_it_stays() {
    let (mut t, _rx) = term(40, 10);
    // A one-line prompt (bash's default): the mark is on the cursor's own row.
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07first\r\nlast\r\n");
    t.write(b"\x1b]133;D;0\x07\x1b]133;A\x07$ \x1b]133;B\x07");
    t.take_prompt_off();
    let kept = export(&t, TextFormat::Plain);
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
