//! Styled rows read by screen row: the same cells a frame's walk gives for the viewport, and
//! the rows scrolled above it too.

use temor_vt::{ColorRef, StyledCell};

use crate::support::{Cell, fixture, row_cells, term};

fn owned(c: &StyledCell) -> Cell {
    Cell {
        text: c.text.clone(),
        width: c.width,
        fg: c.fg,
        bg: c.bg,
        style: c.style,
        linked: c.linked,
        semantic: c.semantic,
    }
}

#[test]
fn a_viewport_row_reads_as_the_frame_draws_it() {
    for (name, cols, rows) in [
        ("sgr.bin", 40, 4),
        ("shell.bin", 80, 24),
        ("graphemes.bin", 40, 6),
    ] {
        let (mut t, _rx) = term(cols, rows);
        t.write(&fixture(name));
        let top = u32::try_from(t.scrollbar().expect("scrollbar").offset.0).expect("top");
        for row in 0..rows {
            let framed = row_cells(&mut t, row);
            let styled: Vec<Cell> = t
                .styled_row(top + u32::from(row))
                .expect("styled")
                .iter()
                .map(owned)
                .collect();
            assert_eq!(styled, framed, "{name} row {row}");
        }
    }
}

#[test]
fn a_row_scrolled_out_of_the_viewport_keeps_its_text_and_colour() {
    let (mut t, _rx) = term(20, 4);
    t.write(b"\x1b[31mred\x1b[0m\r\n");
    for n in 0..30 {
        t.write(format!("line {n}\r\n").as_bytes());
    }
    let row = t.styled_row(0).expect("styled");
    let text: String = row.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "red");
    assert_eq!(row[0].style.fg_ref, ColorRef::Palette(1));
    assert!(row[0].fg.is_some(), "palette entry 1 resolves to channels");
    let rows = t.styled_rows(1..3).expect("rows");
    let texts: Vec<String> = rows
        .iter()
        .map(|r| r.iter().map(|c| c.text.as_str()).collect())
        .collect();
    assert_eq!(texts, ["line 0", "line 1"]);
}
