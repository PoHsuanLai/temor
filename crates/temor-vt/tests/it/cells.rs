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
