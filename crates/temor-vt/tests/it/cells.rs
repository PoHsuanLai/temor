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
fn the_default_cursor_shape_can_be_set() {
    let (mut t, _events) = term(10, 2);
    t.set_default_cursor_shape(Some(CursorShape::Bar))
        .expect("bar");
    t.set_default_cursor_shape(None)
        .expect("reset to the built-in block");
}
