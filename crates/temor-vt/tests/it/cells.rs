//! Cells of a screen row: wide characters take two columns, with a spacer cell after them, and
//! the default cursor shape can be set.

use temor_vt::{CellWidth, CursorShape};

use crate::support::term;

#[test]
fn a_wide_character_takes_two_cells_with_a_spacer() {
    let (mut t, _events) = term(10, 2);
    t.write("中文 ab".as_bytes());
    let cells = t.row_cells(0).expect("cells");
    let texts: Vec<&str> = cells.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(texts[0], "中");
    assert_eq!(texts[2], "文");
    assert_eq!(
        texts[5], "a",
        "column 5 is the a, after the two wide characters and a space"
    );
    assert_eq!(cells[0].width, CellWidth::Wide);
    assert_eq!(cells[1].width, CellWidth::SpacerTail);
    assert_eq!(cells[2].width, CellWidth::Wide);
    assert_eq!(cells[3].width, CellWidth::SpacerTail);
    assert_eq!(cells[4].width, CellWidth::Narrow);
}

#[test]
fn a_tab_leaves_blank_cells_that_read_as_spaces() {
    let (mut t, _events) = term(12, 2);
    t.write(b"a\tb");
    let texts: Vec<String> = t
        .row_cells(0)
        .expect("cells")
        .into_iter()
        .map(|c| c.text)
        .collect();
    assert_eq!(texts[0], "a");
    assert_eq!(
        texts[1..8].concat(),
        "       ",
        "seven blank cells up to the tab stop"
    );
    assert_eq!(texts[8], "b");
}

#[test]
fn the_default_cursor_shape_can_be_set() {
    let (mut t, _events) = term(10, 2);
    t.set_default_cursor_shape(Some(CursorShape::Bar))
        .expect("bar");
    t.set_default_cursor_shape(None)
        .expect("reset to the built-in block");
}

#[test]
fn the_default_cursor_blink_applies_at_a_style_reset_and_programs_can_change_it() {
    let (mut t, _events) = term(10, 2);
    let mut view = temor_vt::RenderView::new().expect("view");
    let mut blink = |t: &mut temor_vt::Terminal| {
        let frame = view.begin(t).expect("begin").end().expect("end");
        let blink = frame.cursor().map(|c| c.blinking);
        frame.mark_drawn().expect("drawn");
        blink
    };
    assert_eq!(blink(&mut t), Some(false), "libghostty's own default");
    t.set_default_cursor_blink(Some(true)).expect("blink");
    t.write(b"\x1b[0 q");
    assert_eq!(blink(&mut t), Some(true), "the default, applied");
    // (what the program sends, the cursor after it)
    for (seq, want) in [
        (&b"\x1b[2 q"[..], false),
        (b"\x1b[5 q", true),
        (b"\x1b[?12l", false),
        (b"\x1b[0 q", true),
    ] {
        t.write(seq);
        assert_eq!(
            blink(&mut t),
            Some(want),
            "{:?}",
            String::from_utf8_lossy(seq)
        );
    }
    t.set_default_cursor_blink(None).expect("reset");
}
