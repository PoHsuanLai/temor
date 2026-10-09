//! Notices on a fake clock and a recording notifier.

use std::path::PathBuf;

use temor_core::block::Outcome;
use temor_core::notify::Focus;
use temor_ui::pane::PaneEvent;
use temor_vt::Notification;

use super::super::support::tools_present;
use super::support::{hold, pump, shell_path, start};

#[test]
fn a_long_command_notifies_once_when_the_pane_is_unfocused() {
    if !tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-notify", &shell);
    live.session.set_focus(Focus::Unfocused);
    hold(&live);
    pump(&mut live, "the command to start", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 == Outcome::Running)
    });
    // Twelve seconds pass on the fake clock while the command runs.
    live.clock.advance(12_000);
    std::fs::write(&live.go, b"").expect("release");
    pump(&mut live, "the command to finish", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    let sent = live.notifier.sent();
    assert_eq!(sent.len(), 1, "one notice: {sent:?}");
    assert_eq!(sent[0].title, "Command finished");
    assert!(
        sent[0].body.ends_with("finished after 12.0 s"),
        "{:?}",
        sent[0].body
    );
}

#[test]
fn a_long_command_while_focused_and_a_short_one_while_unfocused_stay_quiet() {
    if !tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-quiet", &shell);
    live.session.set_focus(Focus::Focused);
    hold(&live);
    pump(&mut live, "the focused command to start", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 == Outcome::Running)
    });
    live.clock.advance(60_000);
    std::fs::write(&live.go, b"").expect("release");
    pump(&mut live, "the focused command to finish", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    live.session.set_focus(Focus::Unfocused);
    hold(&live);
    pump(&mut live, "the short command to start", |s| {
        s.block_outcomes()
            .get(1)
            .is_some_and(|b| b.1 == Outcome::Running)
    });
    live.clock.advance(2_000);
    std::fs::write(&live.go, b"").expect("release");
    pump(&mut live, "the short command to finish", |s| {
        s.block_outcomes()
            .get(1)
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    assert!(live.notifier.sent().is_empty());
}

#[test]
fn program_notifications_and_the_bell_reach_the_edge_rate_limited() {
    let shell = shell_path("bash").unwrap_or_else(|| PathBuf::from("/usr/bin/bash"));
    if !tools_present() {
        return;
    }
    let mut live = start("blocks-program", &shell);
    live.session.set_focus(Focus::Unfocused);
    let note = |body: &str| {
        PaneEvent::Notification(Notification {
            title: String::new(),
            body: body.to_owned(),
        })
    };
    live.session.absorb(note("first"));
    live.session.absorb(note("too soon"));
    live.clock.advance(2_000);
    live.session.absorb(note("later"));
    live.session.absorb(PaneEvent::Bell);
    let sent = live.notifier.sent();
    assert_eq!(
        sent.iter().map(|n| n.body.as_str()).collect::<Vec<_>>(),
        vec!["first", "later"],
        "one per two seconds"
    );
    assert!(live.session.bell_showing(), "the bell's mark is up");
    live.clock.advance(1_000);
    assert!(!live.session.bell_showing(), "and goes down on its own");
}
