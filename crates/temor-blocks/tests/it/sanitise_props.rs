//! Property tests for proposal sanitising: the result is always one safe line, sanitising is
//! idempotent, and `Unchanged` means exactly that. The oracle is the Unicode property itself
//! (ICU's data), never the sanitiser's own table.

use icu_properties::props::{DefaultIgnorableCodePoint, GeneralCategory};
use icu_properties::{CodePointMapData, CodePointSetData};
use proptest::prelude::*;
use temor_blocks::proposal::{Sanitised, sanitise};

/// Every Default_Ignorable_Code_Point and every space separator but U+0020, from ICU.
fn ucd_suspects() -> Vec<char> {
    let ignorable = CodePointSetData::new::<DefaultIgnorableCodePoint>().iter_ranges();
    let spaces = (0u32..=0x10ffff).filter_map(char::from_u32).filter(|c| {
        CodePointMapData::<GeneralCategory>::new().get(*c) == GeneralCategory::SpaceSeparator
    });
    ignorable
        .flat_map(|range| range.filter_map(char::from_u32))
        .chain(spaces)
        .filter(|c| *c != ' ')
        .collect()
}

/// Text mixing plain characters with every kind the sanitiser removes.
fn hostile() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        4 => "[a-z ~/;&|-]{1,6}",
        1 => Just("\u{1b}[201~".to_string()),
        1 => Just("\u{1b}]0;x\u{07}".to_string()),
        1 => Just("\u{9b}2J".to_string()),
        2 => prop::sample::select(ucd_suspects()).prop_map(String::from),
        1 => any::<char>().prop_map(String::from),
    ];
    prop::collection::vec(piece, 0..20).prop_map(|pieces| pieces.concat())
}

fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{2028}' | '\u{2029}' | '\u{fff9}'..='\u{fffb}')
        || CodePointSetData::new::<DefaultIgnorableCodePoint>().contains(c)
        || (c != ' '
            && CodePointMapData::<GeneralCategory>::new().get(c) == GeneralCategory::SpaceSeparator)
}

proptest! {
    #[test]
    fn the_result_is_one_line_with_nothing_unsafe(raw in hostile()) {
        let (line, _) = sanitise(&raw);
        prop_assert!(!line.as_str().chars().any(is_unsafe), "{:?}", line.as_str());
    }

    #[test]
    fn sanitising_twice_changes_nothing_more(raw in hostile()) {
        let (once, _) = sanitise(&raw);
        let (twice, report) = sanitise(once.as_str());
        prop_assert_eq!(twice, once);
        prop_assert_eq!(report, Sanitised::Unchanged);
    }

    #[test]
    fn unchanged_means_the_text_is_kept_whole(raw in hostile()) {
        let (line, report) = sanitise(&raw);
        prop_assert_eq!(report == Sanitised::Unchanged, line.as_str() == raw);
    }
}

#[test]
fn every_unicode_suspect_is_removed_or_made_plain() {
    for c in ucd_suspects() {
        let (line, report) = sanitise(&format!("a{c}b"));
        assert!(
            !line.as_str().chars().any(is_unsafe) && report != Sanitised::Unchanged,
            "U+{:04X} left {:?}",
            u32::from(c),
            line.as_str()
        );
    }
}
