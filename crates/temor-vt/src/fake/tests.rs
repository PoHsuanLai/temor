use std::sync::mpsc;

use super::FakeVt;
use crate::{
    ExitCode, ExitStatus, GridSize, ModeState, PromptEvent, TerminalConfig, TerminalMode,
    UnknownKind, Vt, VtEvent,
};

fn fake(cols: u16, rows: u16) -> (FakeVt, mpsc::Receiver<VtEvent>) {
    let (tx, rx) = mpsc::channel();
    let (replies, _) = mpsc::channel::<Vec<u8>>();
    let config = TerminalConfig::new(GridSize::new(cols, rows));
    (FakeVt::new(config, Box::new(tx), Box::new(replies)), rx)
}

#[test]
fn text_wraps_scrolls_and_reads_back() {
    let (mut vt, _rx) = fake(5, 2);
    vt.write(b"abcdefg\r\nxy\r\nz");
    assert_eq!(vt.plain_text().as_deref(), Ok("abcdefg\nxy\nz"));
}

#[test]
fn osc_title_cwd_and_prompt_marks_become_events() {
    let (mut vt, rx) = fake(20, 4);
    vt.write(b"\x1b]2;one\x07\x1b]7;file://h/tmp\x1b\\\x1b]133;D;3\x07\x1b]2;hi\x07\x07");
    let got: Vec<VtEvent> = rx.try_iter().collect();
    // Titles are merged and sent once the write is done, as libghostty forces on the real one.
    assert_eq!(
        got,
        [
            VtEvent::Cwd("file://h/tmp".into()),
            VtEvent::Prompt(PromptEvent::CommandEnd {
                exit: ExitStatus::Code(ExitCode(3)),
                error: String::new()
            }),
            VtEvent::Bell,
            VtEvent::Title("hi".into()),
        ]
    );
    assert_eq!(vt.title(), "hi");
}

#[test]
fn inline_files_are_taken_out_in_order() {
    let (mut vt, rx) = fake(20, 4);
    vt.write(b"\x07\x1b]1337;File=:QQ==\x07\x07");
    let kinds: Vec<String> = rx
        .try_iter()
        .map(|e| match e {
            VtEvent::Unknown {
                kind: UnknownKind::Osc1337File,
                ..
            } => "file".to_owned(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["Bell", "file", "Bell"]);
}

#[test]
fn program_status_becomes_an_event_and_a_bad_report_none() {
    let (mut vt, rx) = fake(20, 4);
    vt.write(
        b"\x1b]7501;state=working:progress=30:app=cargo\x07\x1b]7501;state=nope\x07\x1b]7501;?\x07",
    );
    let got: Vec<VtEvent> = rx.try_iter().collect();
    let [VtEvent::ProgramStatus(status)] = got.as_slice() else {
        panic!("expected one status, got {got:?}");
    };
    assert_eq!(
        (status.state, status.app.as_str()),
        (crate::ProgramState::Working, "cargo")
    );
}

#[test]
fn dec_modes_toggle() {
    let (mut vt, _rx) = fake(20, 4);
    assert_eq!(vt.mode(TerminalMode::BracketedPaste), Ok(ModeState::Reset));
    vt.write(b"\x1b[?2004h");
    assert_eq!(vt.mode(TerminalMode::BracketedPaste), Ok(ModeState::Set));
    vt.write(b"\x1b[?2027l");
    assert_eq!(vt.mode(TerminalMode::GraphemeCluster), Ok(ModeState::Reset));
}

#[test]
fn full_reset_clears_title_and_reports_reset() {
    let (mut vt, rx) = fake(20, 4);
    vt.write(b"\x1b]2;t\x07\x1b]7;file://h/x\x07");
    vt.write(b"abc\x1b]2;late\x07\x1bc");
    let got: Vec<VtEvent> = rx.try_iter().collect();
    assert_eq!(got.last(), Some(&VtEvent::Reset));
    assert!(
        !got.contains(&VtEvent::Title("late".into())),
        "a reset drops a pending title"
    );
    assert_eq!((vt.title(), vt.pwd()), (String::new(), String::new()));
    assert_eq!(vt.plain_text().as_deref(), Ok(""));
}
