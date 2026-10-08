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
