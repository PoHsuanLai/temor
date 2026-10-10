//! How much a find costs on a big scrollback (FOLLOWUPS: "~3 FFI calls a cell"). Ignored by
//! default: run with `cargo test -p temor-pane --test it search_bench -- --ignored --nocapture`.
//! It prints the time to read every row cell by cell (what the find bar does), to search the rows
//! read, and, for comparison, to format the whole screen as one block of text. If the first
//! dominates, the find bar should read rows through a text call instead.

use std::time::Instant;

use temor_pane::Term;
use temor_pane::text::{RowText, find_all};
use temor_render::Palette;
use temor_vt::{CellPx, CursorShape, GridPoint, GridSize, LineCount, PointSpace};

const LINES: u32 = 100_000;
const COLS: u16 = 120;

#[test]
#[ignore = "a measurement, not a check: run it by hand"]
fn searching_a_hundred_thousand_lines_prints_where_the_time_goes() {
    let mut term = Term::new(
        GridSize::new(COLS, 40),
        CellPx::new(8, 16),
        LineCount(LINES as usize),
        CursorShape::Block,
        &Palette::default(),
    )
    .expect("terminal");
    for batch in 0..(LINES / 1000) {
        let mut text = String::new();
        for i in 0..1000 {
            let n = batch * 1000 + i;
            text.push_str(&format!(
                "line {n} the quick brown fox jumps over the lazy dog 0123456789\r\n"
            ));
        }
        term.write(text.as_bytes());
    }
    let rows = term.screen_rows().expect("rows");
    assert!(rows > LINES / 2, "the scrollback kept {rows} rows");

    let start = Instant::now();
    let lines: Vec<(u32, RowText)> = (0..rows)
        .map(|row| term.row_text(row).map(|line| (row, line)))
        .collect::<Result<_, _>>()
        .expect("rows read");
    let read = start.elapsed();

    let start = Instant::now();
    let found = find_all(
        lines.iter().map(|(row, line)| (*row, line)),
        "lazy dog 0123",
    );
    let searched = start.elapsed();
    assert!(!found.is_empty());

    let start = Instant::now();
    let text = term
        .text(
            GridPoint::new(PointSpace::Screen, 0, 0),
            GridPoint::new(PointSpace::Screen, COLS - 1, rows - 1),
        )
        .expect("text");
    let formatted = start.elapsed();

    println!(
        "{rows} rows: read cell by cell {read:?}, searched {searched:?} ({} matches), \
         formatted as one text {formatted:?} ({} bytes)",
        found.len(),
        text.len()
    );
}
