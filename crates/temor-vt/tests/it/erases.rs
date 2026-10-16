//! Erases (PLAN §8.1): a running command's `CSI 3 J` takes only its own rows, the person's
//! `clear` takes everything, and a `CSI 2 J` that blanks rows in place ends the anchors there.

use std::sync::mpsc;

use temor_vt::{AnchorId, PointSpace, Terminal, VtEvent};

use super::anchors::{anchors, term_with_scrollback};
use crate::support::drain;

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
fn erasing_below_the_cursor_keeps_every_anchor() {
    let (mut t, rx) = term_with_scrollback(20, 6, 1000);
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07claude\r\n\x1b]133;C\x07");
    let ids = anchors(&drain(&rx));
    t.write(b"frame 1\r\nframe 2\r\n\x1b[3A\r\x1b[J");
    for (_, id) in &ids {
        assert!(t.anchor_point(*id, PointSpace::Screen).is_some());
    }
}

/// An earlier block, then a prompt being typed at, all on a screen with room to spare.
fn earlier_then_prompt(
    t: &mut Terminal,
    rx: &mpsc::Receiver<VtEvent>,
) -> (Vec<AnchorId>, AnchorId) {
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07echo hi\r\n\x1b]133;C\x07hi\r\n\x1b]133;D;0\x07");
    let earlier = anchors(&drain(rx)).into_iter().map(|(_, id)| id).collect();
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07");
    let prompt = anchors(&drain(rx))[0].1;
    (earlier, prompt)
}

#[test]
fn the_shells_screen_clear_at_a_prompt_ends_the_blocks_it_blanked_and_keeps_the_prompt_at_the_top()
{
    // Ctrl+L: the screen is blanked in place (it does not end at a prompt), then the shell draws
    // its prompt again from the top, input mark and all.
    let (mut t, rx) = term_with_scrollback(20, 12, 1000);
    let (earlier, prompt) = earlier_then_prompt(&mut t, &rx);
    t.write(b"\x1b[H\x1b[2J$ \x1b]133;B\x07");
    for id in &earlier {
        assert_eq!(t.anchor_point(*id, PointSpace::Screen), None);
    }
    let at = t
        .anchor_point(prompt, PointSpace::Active)
        .expect("the prompt stays");
    assert_eq!((at.col.0, at.row), (0, 0));
}

#[test]
fn a_running_commands_screen_clear_ends_the_blocks_it_blanked_and_its_output_goes_on_from_the_top()
{
    // `clear -x`, or `tput clear` in a script: the command's own line is blanked too.
    let (mut t, rx) = term_with_scrollback(20, 12, 1000);
    let (earlier, _) = earlier_then_prompt(&mut t, &rx);
    t.write(b"clear\r\n\x1b]133;C\x07");
    let output = anchors(&drain(&rx))[0].1;
    t.write(b"\x1b[H\x1b[2Jafter\r\n");
    for id in &earlier {
        assert_eq!(t.anchor_point(*id, PointSpace::Screen), None);
    }
    let at = t
        .anchor_point(output, PointSpace::Active)
        .expect("the output stays");
    assert_eq!((at.col.0, at.row), (0, 0));
}

#[test]
fn a_screen_clear_that_pushes_the_screen_into_the_scrollback_keeps_every_anchor() {
    // The screen ends at a prompt, so libghostty scrolls it into the scrollback first.
    let (mut t, rx) = term_with_scrollback(20, 4, 1000);
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07a\r\nb\r\n\x1b]133;D;0\x07");
    let earlier: Vec<AnchorId> = anchors(&drain(&rx)).into_iter().map(|(_, id)| id).collect();
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07\x1b[H\x1b[2J");
    for id in &earlier {
        assert!(t.anchor_point(*id, PointSpace::Screen).is_some());
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
