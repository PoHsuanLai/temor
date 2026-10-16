//! Anchors (PLAN §8.1, §9): each OSC 133 mark is announced with its number, its own events
//! follow, then its anchor at the cursor. The anchor follows its cell, and the scrollback dropping
//! the cell ends the anchor.

use std::sync::mpsc;

use temor_vt::{
    AnchorId, GridSize, LineCount, PointSpace, PromptEvent, PromptKind, Terminal, TerminalConfig,
    VtEvent,
};

use crate::support::drain;

fn term_with_scrollback(
    cols: u16,
    rows: u16,
    scrollback: usize,
) -> (Terminal, mpsc::Receiver<VtEvent>) {
    let (tx, events) = mpsc::channel();
    let (reply_tx, _replies) = mpsc::channel();
    let mut config = TerminalConfig::new(GridSize::new(cols, rows));
    config.scrollback = LineCount(scrollback);
    let term = Terminal::new(config, Box::new(tx), Box::new(reply_tx)).expect("terminal");
    (term, events)
}

/// The anchors the events carried, in order, with their mark numbers.
fn anchors(events: &[VtEvent]) -> Vec<(u64, AnchorId)> {
    events
        .iter()
        .filter_map(|e| match e {
            VtEvent::Anchor { seq, anchor } => Some((*seq, *anchor)),
            _ => None,
        })
        .collect()
}

#[test]
fn each_mark_is_announced_then_anchored_at_the_cursor_after_its_events() {
    let (mut t, rx) = term_with_scrollback(20, 5, 100);
    t.write(b"ab\x1b]133;A\x07cd\x1b]133;B\x07");
    let events = drain(&rx);
    let ids = anchors(&events);
    assert_eq!(
        ids.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        events,
        vec![
            VtEvent::MarkBegin { seq: 1 },
            VtEvent::Prompt(PromptEvent::PromptStart(PromptKind::Primary)),
            VtEvent::Anchor {
                seq: 1,
                anchor: ids[0].1
            },
            VtEvent::MarkBegin { seq: 2 },
            VtEvent::Prompt(PromptEvent::InputStart),
            VtEvent::Anchor {
                seq: 2,
                anchor: ids[1].1
            },
        ]
    );
    // OSC 133 A starts on a fresh line (libghostty's fresh-line), so its anchor is column 0 of
    // the next row; B is where the typed text now ends.
    let at = |id: AnchorId| {
        t.anchor_point(id, PointSpace::Active)
            .map(|p| (p.col.0, p.row))
    };
    assert_eq!(at(ids[0].1), Some((0, 1)));
    assert_eq!(at(ids[1].1), Some((2, 1)));
}

#[test]
fn a_mark_split_across_writes_is_anchored_once_it_completes() {
    let (mut t, rx) = term_with_scrollback(20, 5, 100);
    t.write(b"x\x1b]133;");
    assert!(
        anchors(&drain(&rx)).is_empty(),
        "no anchor before the mark ends"
    );
    t.write(b"C\x1b\\y");
    let events = drain(&rx);
    assert_eq!(anchors(&events).len(), 1);
    let at = t
        .anchor_point(anchors(&events)[0].1, PointSpace::Active)
        .expect("anchored");
    assert_eq!((at.col.0, at.row), (1, 0));
}

#[test]
fn a_non_mark_osc_is_not_announced() {
    let (mut t, rx) = term_with_scrollback(20, 5, 100);
    t.write(b"\x1b]0;title\x07\x1b]7;file://h/tmp\x1b\\");
    assert!(
        drain(&rx)
            .iter()
            .all(|e| !matches!(e, VtEvent::MarkBegin { .. } | VtEvent::Anchor { .. }))
    );
}

#[test]
fn an_anchor_follows_its_cell_as_output_scrolls() {
    let (mut t, rx) = term_with_scrollback(20, 3, 100);
    t.write(b"top\x1b]133;A\x07\r\n");
    let id = anchors(&drain(&rx))[0].1;
    let before = t.anchor_point(id, PointSpace::Screen).expect("anchored");
    t.write(b"l1\r\nl2\r\nl3\r\nl4\r\n");
    let after = t
        .anchor_point(id, PointSpace::Screen)
        .expect("still anchored");
    assert_eq!(before.col, after.col);
    assert_eq!(
        after.row, before.row,
        "the anchored row is the same row of history"
    );
    assert!(
        t.anchor_point(id, PointSpace::Active).is_none(),
        "it scrolled into history"
    );
}

#[test]
fn scrollback_pruning_ends_the_anchor() {
    let (mut t, rx) = term_with_scrollback(20, 2, 4);
    t.write(b"first\x1b]133;A\x07\r\n");
    let id = anchors(&drain(&rx))[0].1;
    assert!(t.anchor_point(id, PointSpace::Screen).is_some());
    // Scrollback is kept in pages, so only a long run of output drops the anchored row.
    for i in 0..20_000 {
        t.write(format!("line {i}\r\n").as_bytes());
    }
    assert!(
        t.anchor_point(id, PointSpace::Screen).is_none(),
        "the scrollback dropped the anchored row"
    );
}

#[test]
fn a_released_anchor_has_no_point_and_releasing_twice_is_harmless() {
    let (mut t, rx) = term_with_scrollback(20, 5, 100);
    t.write(b"a\x1b]133;A\x07");
    let id = anchors(&drain(&rx))[0].1;
    t.release_anchor(id);
    assert!(t.anchor_point(id, PointSpace::Active).is_none());
    t.release_anchor(id);
}

#[test]
fn anchor_cursor_works_without_a_mark_and_is_counted() {
    let (mut t, _rx) = term_with_scrollback(20, 5, 100);
    t.write(b"hello");
    assert_eq!(t.anchor_count(), 0);
    let id = t.anchor_cursor().expect("anchored");
    assert_eq!(t.anchor_count(), 1);
    let at = t.anchor_point(id, PointSpace::Active).expect("point");
    assert_eq!((at.col.0, at.row), (5, 0));
    t.release_anchor(id);
    assert_eq!(t.anchor_count(), 0);
}

#[test]
fn the_alternate_screen_is_reported_while_a_full_screen_program_runs() {
    let (mut t, _rx) = term_with_scrollback(20, 5, 100);
    assert!(!t.on_alternate_screen());
    t.write(b"\x1b[?1049h");
    assert!(t.on_alternate_screen());
    t.write(b"\x1b[?1049l");
    assert!(!t.on_alternate_screen());
}

/// Everything the terminal keeps, scrollback and screen, as plain text.
fn kept_text(t: &Terminal) -> String {
    temor_vt::Formatter::new(temor_vt::TextFormat::Plain)
        .format(t, temor_vt::TextSpan::Screen)
        .expect("format")
}

/// A finished command (`ls`, ten lines of output) then the prompt of a second, `claude`, whose
/// output has started. The anchors of both, in mark order.
fn earlier_then_running(
    t: &mut Terminal,
    rx: &mpsc::Receiver<VtEvent>,
) -> (Vec<AnchorId>, Vec<AnchorId>) {
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07");
    for i in 0..10 {
        t.write(format!("old {i}\r\n").as_bytes());
    }
    t.write(b"\x1b]133;D;0\x07");
    let earlier = anchors(&drain(rx)).into_iter().map(|(_, id)| id).collect();
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07claude\r\n\x1b]133;C\x07");
    let running = anchors(&drain(rx)).into_iter().map(|(_, id)| id).collect();
    (earlier, running)
}

#[test]
fn a_running_commands_scrollback_erase_takes_only_its_own_rows() {
    // A frame taller than the screen, then the screen and the scrollback cleared, as Claude Code
    // redraws; the erase arrives whole or split between two writes.
    for split in [false, true] {
        let (mut t, rx) = term_with_scrollback(20, 6, 1000);
        let (earlier, running) = earlier_then_running(&mut t, &rx);
        for i in 0..8 {
            t.write(format!("frame {i}\r\n").as_bytes());
        }
        if split {
            t.write(b"\x1b[2J\x1b[3");
            t.write(b"J\x1b[Hbye");
        } else {
            t.write(b"\x1b[2J\x1b[3J\x1b[Hbye");
        }
        let text = kept_text(&t);
        assert!(
            text.contains("old 0") && text.contains("old 9"),
            "{split}: {text}"
        );
        assert!(
            text.contains("claude"),
            "{split}: the command line stays: {text}"
        );
        assert!(
            !text.contains("frame"),
            "{split}: the command's own rows go: {text}"
        );
        assert!(text.contains("bye"), "{split}: {text}");
        for id in earlier.iter().chain(&running[..2]) {
            assert!(t.anchor_point(*id, PointSpace::Screen).is_some(), "{split}");
        }
        let output = t.anchor_point(running[2], PointSpace::Active);
        assert_eq!(
            output.map(|p| (p.row, p.col.0)),
            Some((0, 0)),
            "{split}: the output mark moves to the first row left"
        );
    }
}

#[test]
fn a_scrollback_erase_by_a_command_still_on_the_screen_erases_nothing_and_prints_nothing() {
    let (mut t, rx) = term_with_scrollback(20, 6, 1000);
    earlier_then_running(&mut t, &rx);
    t.write(b"one\r\ntwo");
    let before = kept_text(&t);
    t.write(b"\x1b[3J");
    assert_eq!(kept_text(&t), before);
    t.write(b"!");
    assert!(kept_text(&t).contains("two!"), "the cursor did not move");
}

#[test]
fn at_a_prompt_or_after_the_persons_clear_the_scrollback_erase_takes_everything() {
    let cases: [(&str, &[u8]); 2] = [
        ("at a prompt", b"\x1b]133;D;0\x07\x1b]133;A\x07$ \x1b[3J"),
        (
            "the person's clear",
            b"\x1b]7502;clear\x07\x1b[H\x1b[2J\x1b[3J",
        ),
    ];
    for (name, tail) in cases {
        let (mut t, rx) = term_with_scrollback(20, 6, 1000);
        let (earlier, _) = earlier_then_running(&mut t, &rx);
        for i in 0..8 {
            t.write(format!("frame {i}\r\n").as_bytes());
        }
        t.write(tail);
        let text = kept_text(&t);
        assert!(!text.contains("old 0"), "{name}: {text}");
        for id in &earlier {
            assert_eq!(t.anchor_point(*id, PointSpace::Screen), None, "{name}");
        }
    }
}

#[test]
fn the_persons_clear_ends_the_anchors_still_on_the_screen() {
    // A short screen that does not end at a prompt: libghostty clears it in place.
    let (mut t, rx) = term_with_scrollback(20, 12, 1000);
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07echo hi\r\n\x1b]133;C\x07hi\r\n\x1b]133;D;0\x07");
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07clear\r\n\x1b]133;C\x07");
    let ids = anchors(&drain(&rx));
    t.write(b"\x1b]7502;clear\x07\x1b[H\x1b[2J\x1b[3J");
    for (_, id) in &ids {
        assert_eq!(t.anchor_point(*id, PointSpace::Screen), None);
    }
}

#[test]
fn erasing_only_the_screen_or_below_the_cursor_keeps_every_anchor() {
    for (name, clear) in [
        ("2J", &b"\x1b[2J\x1b[H"[..]),
        ("up and erase", &b"\x1b[3A\r\x1b[J"[..]),
    ] {
        let (mut t, rx) = term_with_scrollback(20, 6, 1000);
        t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07claude\r\n\x1b]133;C\x07");
        let ids = anchors(&drain(&rx));
        t.write(b"frame 1\r\nframe 2\r\n");
        t.write(clear);
        for (_, id) in &ids {
            assert!(t.anchor_point(*id, PointSpace::Screen).is_some(), "{name}");
        }
    }
}

#[test]
fn a_full_reset_ends_every_anchor() {
    let (mut t, rx) = term_with_scrollback(20, 6, 1000);
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07claude\r\n\x1b]133;C\x07");
    let ids = anchors(&drain(&rx));
    for i in 0..8 {
        t.write(format!("frame {i}\r\n").as_bytes());
    }
    t.write(b"\x1b]133;A\x07$ ");
    let on_screen = anchors(&drain(&rx));
    t.write(b"\x1bc");
    for (_, id) in ids.iter().chain(&on_screen) {
        assert_eq!(t.anchor_point(*id, PointSpace::Screen), None);
    }
}
