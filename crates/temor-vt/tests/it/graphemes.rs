//! `graphemes.bin` and `graphemes_2027_off.bin`: wide CJK, combining marks, emoji with skin
//! tone and ZWJ sequences, with grapheme clustering (mode 2027) on by default and off.

use temor_vt::{CellWidth, ModeState, TerminalMode};

use crate::support::{fixture, row_cells, row_texts, term};

#[test]
fn grapheme_cluster_mode_is_on_by_default() {
    let (t, _rx) = term(40, 4);
    assert_eq!(t.mode(TerminalMode::GraphemeCluster), Ok(ModeState::Set));
}

#[test]
fn wide_chars_and_emoji_take_one_wide_cell_each() {
    let (mut t, _rx) = term(40, 4);
    t.write(&fixture("graphemes.bin"));
    let cells = row_cells(&mut t, 0);
    let shown: Vec<(&str, CellWidth)> = cells
        .iter()
        .filter(|c| c.width != CellWidth::SpacerTail)
        .take(10)
        .map(|c| (c.text.as_str(), c.width))
        .collect();
    assert_eq!(
        shown,
        [
            ("中", CellWidth::Wide),
            ("文", CellWidth::Wide),
            (" ", CellWidth::Narrow),
            ("e\u{301}", CellWidth::Narrow),
            (" ", CellWidth::Narrow),
            ("👍🏽", CellWidth::Wide),
            (" ", CellWidth::Narrow),
            ("👨\u{200d}👩\u{200d}👧", CellWidth::Wide),
            (" ", CellWidth::Narrow),
            ("o", CellWidth::Narrow),
        ]
    );
    // Each wide cell is followed by its spacer.
    assert_eq!(cells[1].width, CellWidth::SpacerTail);
}

#[test]
fn without_mode_2027_a_skin_tone_splits_off() {
    let (mut t, _rx) = term(40, 4);
    t.write(&fixture("graphemes_2027_off.bin"));
    assert_eq!(t.mode(TerminalMode::GraphemeCluster), Ok(ModeState::Reset));
    let texts = row_texts(&mut t, 0);
    let bar = texts.iter().position(|s| s == "|").expect("the bar");
    assert!(
        !texts[..bar].iter().any(|s| s == "👍🏽"),
        "clustered without 2027: {texts:?}"
    );
    assert!(
        texts[..bar].iter().any(|s| s.starts_with('👍')),
        "{texts:?}"
    );
}
