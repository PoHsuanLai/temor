//! Selection, search and wide cells: what the pane selects, finds and copies.

use super::row::RowText;
use super::search::{Finder, find_all};
use super::selection::{CellPos, Granularity, Selection, Shape, word_span};

#[test]
fn selection_drags_in_both_directions() {
    let line = RowText::plain("hello world", false);
    let mut sel = Selection::begin(CellPos::new(2, 4), &line, Granularity::Cells, Shape::Linear);
    sel.extend(CellPos::new(2, 1), &line);
    assert_eq!(sel.ends(), (CellPos::new(2, 1), CellPos::new(2, 4)));
    assert!(sel.contains(CellPos::new(2, 2)));
    assert!(!sel.contains(CellPos::new(2, 5)));
    assert_eq!(sel.text(|_| line.clone()), "ello");
}

#[test]
fn selection_spans_rows_and_drops_trailing_blanks() {
    let rows = [
        RowText::plain("first   ", false),
        RowText::plain("second", false),
        RowText::plain("third", false),
    ];
    let mut sel = Selection::begin(
        CellPos::new(0, 3),
        &rows[0],
        Granularity::Cells,
        Shape::Linear,
    );
    sel.extend(CellPos::new(2, 1), &rows[2]);
    assert_eq!(sel.text(|r| rows[r as usize].clone()), "st\nsecond\nth");
}

#[test]
fn word_and_line_selection_snap_to_their_runs() {
    let line = RowText::plain("foo_bar baz", false);
    assert_eq!(word_span(&line, 2), (0, 6));
    assert_eq!(word_span(&RowText::plain("foo  baz", false), 4), (3, 4));
    assert_eq!(word_span(&RowText::plain("a.b", false), 1), (1, 1));
    let run = RowText::plain("run ls -la now", false);
    let mut sel = Selection::begin(CellPos::new(0, 4), &run, Granularity::Words, Shape::Linear);
    assert_eq!(sel.text(|_| run.clone()), "ls");
    sel.extend(CellPos::new(0, 9), &run);
    assert_eq!(sel.text(|_| run.clone()), "ls -la");
    let lines = Selection::begin(CellPos::new(0, 1), &run, Granularity::Lines, Shape::Linear);
    assert_eq!(lines.text(|_| run.clone()), "run ls -la now");
}

#[test]
fn block_selection_takes_a_rectangle() {
    let rows = [
        RowText::plain("abcdef", false),
        RowText::plain("ghijkl", false),
    ];
    let mut sel = Selection::begin(
        CellPos::new(0, 1),
        &rows[0],
        Granularity::Cells,
        Shape::Block,
    );
    sel.extend(CellPos::new(1, 3), &rows[1]);
    assert_eq!(sel.text(|r| rows[r as usize].clone()), "bcd\nhij");
}

#[test]
fn search_is_case_insensitive_and_steps_with_wrap() {
    let rows = [
        RowText::plain("Error one", false),
        RowText::plain("nothing", false),
        RowText::plain("error and ERROR", false),
    ];
    let matches = find_all(rows.iter().enumerate().map(|(i, r)| (i as u32, r)), "error");
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[2].col, 10);
    let mut finder = Finder::new("error", matches);
    assert_eq!(finder.position(), Some((1, 3)));
    finder.previous();
    assert_eq!(finder.position(), Some((3, 3)));
    finder.next();
    assert_eq!(finder.position(), Some((1, 3)));
    assert!(find_all([(0, &RowText::plain("x", false))], "").is_empty());
}

#[test]
fn a_soft_wrapped_row_joins_the_next_without_a_line_break() {
    // "abcdefghij" wrapped at ten columns, then "klmno" on its own row: one line when copied.
    let rows = [
        RowText::plain("abcdefghij", true),
        RowText::plain("klmno", false),
        RowText::plain("next", false),
    ];
    let mut sel = Selection::begin(
        CellPos::new(0, 0),
        &rows[0],
        Granularity::Cells,
        Shape::Linear,
    );
    sel.extend(CellPos::new(2, 3), &rows[2]);
    assert_eq!(
        sel.text(|r| rows[r as usize].clone()),
        "abcdefghijklmno\nnext"
    );
}

#[test]
fn wide_characters_are_two_columns_in_selection_and_search() {
    use super::row::{RowCell, Span};
    let wide = |c: &str| RowCell {
        text: c.to_owned(),
        span: Span::Double,
    };
    let spacer = RowCell {
        text: String::new(),
        span: Span::Spacer,
    };
    let plain = |c: char| RowCell {
        text: c.to_string(),
        span: Span::Single,
    };
    // 中 文 _ a b, as the terminal lays them out: 中 (0-1), 文 (2-3), space (4), a (5), b (6).
    let row = RowText {
        cells: vec![
            wide("中"),
            spacer.clone(),
            wide("文"),
            spacer,
            plain(' '),
            plain('a'),
            plain('b'),
        ],
        wraps: false,
    };
    // A double-click on the a selects "ab" by its cell columns, not by characters.
    assert_eq!(word_span(&row, 5), (5, 6));
    // Selecting cells 0 through 3 copies the two characters once.
    let sel = Selection::begin(CellPos::new(0, 0), &row, Granularity::Cells, Shape::Linear);
    let mut sel = sel;
    sel.extend(CellPos::new(0, 3), &row);
    assert_eq!(sel.text(|_| row.clone()), "中文");
    // Searching finds the characters at their cell columns: 文 starts at column 2.
    let found = find_all([(0, &row)], "文");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].col, 2);
    assert_eq!(found[0].len, 2, "a wide match covers its two columns");
    let ab = find_all([(0, &row)], "ab");
    assert_eq!((ab[0].col, ab[0].len), (5, 2));
}
