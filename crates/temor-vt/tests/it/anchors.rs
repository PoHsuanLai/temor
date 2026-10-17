//! Anchors (PLAN §8.1, §9): each OSC 133 mark is announced with its number, its own events
//! follow, then its anchor at the cursor. The anchor follows its cell, and the scrollback dropping
//! the cell ends the anchor.

use std::sync::mpsc;

use temor_vt::{
    AnchorId, GridSize, LineCount, PointSpace, PromptEvent, PromptKind, Terminal, TerminalConfig,
    VtEvent,
};

use crate::support::drain;

pub(super) fn term_with_scrollback(
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
pub(super) fn anchors(events: &[VtEvent]) -> Vec<(u64, AnchorId)> {
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
fn a_terminal_numbered_after_another_never_reuses_its_anchor_ids() {
    let (mut old, _old_rx) = term_with_scrollback(20, 5, 100);
    let taken = [old.anchor_cursor(), old.anchor_cursor()].map(|id| id.expect("anchored"));
    let (mut new, _new_rx) = term_with_scrollback(20, 5, 100);
    new.number_anchors_after(&old);
    let fresh = new.anchor_cursor().expect("anchored");
    assert!(!taken.contains(&fresh), "{fresh:?} reuses one of {taken:?}");
    for id in taken {
        new.release_anchor(id);
    }
    assert!(new.anchor_point(fresh, PointSpace::Active).is_some());
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
