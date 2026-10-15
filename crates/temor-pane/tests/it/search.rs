//! The find bar glances at blocks of rows as formatted text and reads cell by cell only the
//! blocks that could match: it must find what reading every row finds.

use temor_pane::Term;
use temor_pane::text::{RowText, find_all, may_match};
use temor_render::Palette;
use temor_vt::{CellPx, CursorShape, GridSize, LineCount};

#[test]
fn glancing_at_rows_as_text_finds_what_reading_every_cell_finds() {
    let mut term = Term::new(
        GridSize::new(20, 6),
        CellPx::new(8, 16),
        LineCount(1_000),
        CursorShape::Block,
        &Palette::default(),
    )
    .expect("terminal");
    let lines = [
        "plain words here",
        "日本語 wide 漢字",
        "gap\x1b[10Gafter",
        "end with space   ",
        "a line long enough to wrap onto the next row of twenty",
        "\x1b[1mbold\x1b[0m and \x1b[4munder\x1b[0m",
        "tab\there",
    ];
    for i in 0..40 {
        term.write(format!("{}\r\n", lines[i % lines.len()]).as_bytes());
    }
    let rows = term.screen_rows().expect("rows");
    let every: Vec<(u32, RowText)> = (0..rows)
        .map(|row| (row, term.row_text(row).expect("row")))
        .collect();
    let (mut found_any, mut skipped) = (0, 0);
    for query in [
        "words",
        "本語",
        "語 w",
        "gap",
        "p      a",
        "space ",
        " with",
        "next",
        "wrap onto",
        "bold and",
        "tab",
        "b    h",
        "x",
    ] {
        let all = find_all(every.iter().map(|(r, l)| (*r, l)), query);
        let mut glanced = Vec::new();
        for start in (0..rows).step_by(3) {
            let end = (start + 3).min(rows);
            if may_match(&term.rows_text(start..end).expect("text"), query) {
                glanced.extend(every[start as usize..end as usize].iter().cloned());
            } else {
                skipped += 1;
            }
        }
        let found = find_all(glanced.iter().map(|(r, l)| (*r, l)), query);
        assert_eq!(found, all, "{query:?}");
        found_any += usize::from(!all.is_empty());
    }
    assert!(found_any >= 10, "only {found_any} queries matched anything");
    assert!(skipped > 0, "the glance never skipped a block");
}
