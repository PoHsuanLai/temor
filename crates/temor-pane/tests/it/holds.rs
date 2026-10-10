//! A hold that outlives its write is drawn from the frame captured where it began, until the
//! program releases it or the window ends it.

use temor_pane::{PaintSource, Term};
use temor_render::Palette;
use temor_vt::{CellPx, CursorShape, GridSize, LineCount};

fn term() -> Term {
    Term::new(
        GridSize::new(40, 5),
        CellPx::new(8, 16),
        LineCount(10),
        CursorShape::Block,
        &Palette::default(),
    )
    .expect("terminal")
}

/// The source's kind, for comparing.
fn kind(source: &PaintSource) -> &'static str {
    match source {
        PaintSource::Live { .. } => "live",
        PaintSource::Fresh(_) => "fresh",
        PaintSource::Held => "held",
        PaintSource::Skip => "skip",
        _ => "other",
    }
}

#[test]
fn a_held_screen_is_drawn_from_its_captured_frame_until_it_ends() {
    // (writes, what each paint after them draws from)
    let table: [(&[&[u8]], &[&str]); 3] = [
        (&[b"a\x1b[?2026hb"], &["fresh", "held", "held"]),
        (&[b"a\x1b[?2026hb\x1b[?2026l"], &["live", "live", "live"]),
        (
            &[b"a\x1b[?2026hb", b"c\x1b[?2026l"],
            &["fresh", "live", "live"],
        ),
    ];
    for (writes, want) in table {
        let mut term = term();
        let mut seen = Vec::new();
        for (i, w) in writes.iter().enumerate() {
            term.write(w);
            if i + 1 == writes.len() {
                break;
            }
            seen.push(kind(&term.paint_source()));
        }
        while seen.len() < want.len() {
            seen.push(kind(&term.paint_source()));
        }
        assert_eq!(seen, want, "{writes:?}");
    }
}

#[test]
fn the_window_can_end_a_hold_the_program_never_releases() {
    let mut term = term();
    term.write(b"a\x1b[?2026hb");
    assert_eq!(kind(&term.paint_source()), "fresh");
    term.end_hold().expect("ends");
    assert_eq!(kind(&term.paint_source()), "live");
}
