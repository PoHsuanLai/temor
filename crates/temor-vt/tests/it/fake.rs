//! `FakeVt` through the `Vt` trait, the way other crates' tests drive it.

use std::sync::mpsc;

use temor_vt::{FakeVt, GridSize, PromptEvent, TerminalConfig, Vt, VtEvent};

fn drive(vt: &mut dyn Vt) -> String {
    vt.write(b"\x1b]133;A\x07$ ls\r\nfile\r\n");
    vt.plain_text().expect("text")
}

#[test]
fn fake_runs_behind_the_trait() {
    let (tx, rx) = mpsc::channel();
    let mut fake = FakeVt::new(TerminalConfig::new(GridSize::new(20, 5)), Box::new(tx));
    assert_eq!(drive(&mut fake), "$ ls\nfile");
    assert_eq!(
        rx.try_iter().collect::<Vec<_>>(),
        [VtEvent::Prompt(PromptEvent::PromptStart(
            temor_vt::PromptKind::Primary
        ))]
    );
}

#[cfg(feature = "ghostty")]
#[test]
fn real_and_fake_agree_on_plain_text() {
    let (tx, _rx) = mpsc::channel();
    let mut fake = FakeVt::new(TerminalConfig::new(GridSize::new(20, 5)), Box::new(tx));
    let (mut real, _rx2) = crate::support::term(20, 5);
    assert_eq!(drive(&mut real), drive(&mut fake));
}
