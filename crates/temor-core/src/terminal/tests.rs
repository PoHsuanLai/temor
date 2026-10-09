//! Unit tests for the pane's pure parts.

use ds_core::word::Word;

use super::action::Action;
use super::chord::{Chord, ChordKey, Mods, NamedKey};
use super::keymap::Keymap;
use super::paste::{PasteCheck, PasteMode, UnsafeReason, check, encode};
use super::search::{Finder, find_all};
use super::selection::{CellPos, Granularity, Selection, Shape, word_span};
use super::settings::parse_settings;
use crate::error::CoreError;

fn ctrl_shift(c: char) -> Chord {
    Chord::of_char(c, Mods::CTRL.with(Mods::SHIFT))
}

#[test]
fn chords_parse_in_any_case_and_order_of_modifiers() {
    let parsed = Chord::parse("Shift+Ctrl+C").expect("valid chord");
    assert_eq!(parsed, ctrl_shift('c'));
    assert_eq!(
        Chord::parse("alt+enter").map(|c| c.key),
        Ok(ChordKey::Named(NamedKey::Enter))
    );
    assert!(matches!(Chord::parse(""), Err(CoreError::BadChord(_))));
    assert!(matches!(
        Chord::parse("hyper+c"),
        Err(CoreError::BadChord(_))
    ));
    assert!(matches!(Chord::parse("ctrl+"), Err(CoreError::BadChord(_))));
}

#[test]
fn chord_labels_show_the_command_key_for_ctrl_shift() {
    assert_eq!(ctrl_shift('c').label(), "⌘C");
    assert_eq!(
        Chord::parse("ctrl+alt+g").map(|c| c.label()),
        Ok("⌃⌥G".to_owned())
    );
    assert_eq!(
        Chord::parse("alt+enter").map(|c| c.label()),
        Ok("⌥↩".to_owned())
    );
}

#[test]
fn super_and_ctrl_space_are_reserved() {
    assert!(Chord::parse("super+c").is_ok_and(|c| c.is_reserved()));
    assert!(Chord::parse("ctrl+space").is_ok_and(|c| c.is_reserved()));
    assert!(!ctrl_shift('c').is_reserved());
}

#[test]
fn defaults_bind_each_action_once_and_never_use_super() {
    let keymap = Keymap::defaults();
    for action in Action::ALL {
        let chord = keymap
            .chord_for(*action)
            .expect("every action has a default");
        assert_eq!(keymap.action_for(chord), Some(*action));
        assert!(!chord.is_reserved());
    }
    assert_eq!(keymap.action_for(&ctrl_shift('c')), Some(Action::Copy));
}

#[test]
fn rebinding_moves_an_action_and_refuses_clashes() {
    let mut keymap = Keymap::defaults();
    let alt_c = Chord::parse("ctrl+alt+c").expect("valid");
    keymap.bind(Action::Copy, alt_c).expect("free chord");
    assert_eq!(keymap.action_for(&alt_c), Some(Action::Copy));
    assert_eq!(keymap.action_for(&ctrl_shift('c')), None);
    let clash = keymap.bind(Action::Paste, alt_c);
    assert!(matches!(clash, Err(CoreError::ChordInUse { .. })));
    let reserved = keymap.bind(Action::Paste, Chord::parse("super+v").expect("valid"));
    assert!(matches!(reserved, Err(CoreError::ReservedChord(_))));
}

#[test]
fn settings_text_applies_good_lines_and_reports_bad_ones() {
    let text = "# comment\nfont.size = 15\nscrollback.lines = 500\ncursor.style = bar\n\
                font.family = \"Iosevka\"\nshell.program = fish\nkeys.copy = ctrl+alt+c\n\
                font.size = huge\nkeys.paste = super+v\nbogus = 1\n";
    let parsed = parse_settings(text);
    assert_eq!(parsed.settings.font_size.points(), 15.0);
    assert_eq!(parsed.settings.scrollback.lines(), 500);
    assert_eq!(parsed.settings.keymap.action_for(&ctrl_shift('c')), None);
    let lines: Vec<usize> = parsed.issues.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![8, 9, 10]);
}

#[test]
fn settings_values_have_limits() {
    assert!(parse_settings("font.size = 5").issues.len() == 1);
    assert!(parse_settings("font.size = 72").issues.is_empty());
    assert!(parse_settings("scrollback.lines = 1000001").issues.len() == 1);
    assert!(parse_settings("cursor.style = blink").issues.len() == 1);
    assert_eq!(parse_settings("").settings.scrollback.lines(), 10_000);
}

#[test]
fn paste_is_unsafe_with_a_newline_only_when_not_bracketed() {
    assert_eq!(
        check("ls\n", PasteMode::Plain),
        PasteCheck::NeedsConfirmation(UnsafeReason::Newline)
    );
    assert_eq!(check("ls\n", PasteMode::Bracketed), PasteCheck::Safe);
    let escape = "x\x1b[201~rm -rf ~\n";
    assert_eq!(
        check(escape, PasteMode::Bracketed),
        PasteCheck::NeedsConfirmation(UnsafeReason::EndOfFrame)
    );
}

#[test]
fn paste_frames_and_cleans_bytes() {
    assert_eq!(
        encode("a\x03b", PasteMode::Bracketed),
        b"\x1b[200~a b\x1b[201~".to_vec()
    );
    assert_eq!(encode("a\nb", PasteMode::Plain), b"a\rb".to_vec());
}

#[test]
fn selection_drags_in_both_directions() {
    let line = "hello world";
    let mut sel = Selection::begin(CellPos::new(2, 4), line, Granularity::Cells, Shape::Linear);
    sel.extend(CellPos::new(2, 1), line);
    assert_eq!(sel.ends(), (CellPos::new(2, 1), CellPos::new(2, 4)));
    assert!(sel.contains(CellPos::new(2, 2)));
    assert!(!sel.contains(CellPos::new(2, 5)));
    assert_eq!(sel.text(|_| line.to_owned()), "ello");
}

#[test]
fn selection_spans_rows_and_drops_trailing_blanks() {
    let rows = ["first   ", "second", "third"];
    let mut sel = Selection::begin(
        CellPos::new(0, 3),
        rows[0],
        Granularity::Cells,
        Shape::Linear,
    );
    sel.extend(CellPos::new(2, 1), rows[2]);
    let text = sel.text(|row| rows[row as usize].to_owned());
    assert_eq!(text, "st\nsecond\nth");
}

#[test]
fn word_and_line_selection_snap_to_their_runs() {
    assert_eq!(word_span("foo_bar baz", 2), (0, 6));
    assert_eq!(word_span("foo  baz", 4), (3, 4));
    assert_eq!(word_span("a.b", 1), (1, 1));
    let line = "run ls -la now";
    let mut sel = Selection::begin(CellPos::new(0, 4), line, Granularity::Words, Shape::Linear);
    assert_eq!(sel.text(|_| line.to_owned()), "ls");
    sel.extend(CellPos::new(0, 9), line);
    assert_eq!(sel.text(|_| line.to_owned()), "ls -la");
    let lines = Selection::begin(CellPos::new(0, 1), line, Granularity::Lines, Shape::Linear);
    assert_eq!(lines.text(|_| line.to_owned()), line);
}

#[test]
fn block_selection_takes_a_rectangle() {
    let rows = ["abcdef", "ghijkl"];
    let mut sel = Selection::begin(
        CellPos::new(0, 1),
        rows[0],
        Granularity::Cells,
        Shape::Block,
    );
    sel.extend(CellPos::new(1, 3), rows[1]);
    assert_eq!(sel.text(|row| rows[row as usize].to_owned()), "bcd\nhij");
}

#[test]
fn search_is_case_insensitive_and_steps_with_wrap() {
    let rows = [(0, "Error one"), (1, "nothing"), (2, "error and ERROR")];
    let matches = find_all(rows, "error");
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[2].col, 10);
    let mut finder = Finder::new("error", matches);
    assert_eq!(finder.position(), Some((1, 3)));
    finder.previous();
    assert_eq!(finder.position(), Some((3, 3)));
    finder.next();
    assert_eq!(finder.position(), Some((1, 3)));
    assert!(find_all([(0, "x")], "").is_empty());
}
