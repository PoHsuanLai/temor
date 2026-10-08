//! Property tests for proposal sanitising: the result is always one safe line, sanitising is
//! idempotent, and `Unchanged` means exactly that.

use proptest::prelude::*;
use temor_core::proposal::{Sanitised, sanitise};

/// Text mixing plain characters with every kind the sanitiser removes.
fn hostile() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        4 => "[a-z ~/;&|-]{1,6}",
        1 => Just("\u{1b}[201~".to_string()),
        1 => Just("\u{1b}]0;x\u{07}".to_string()),
        1 => Just("\u{9b}2J".to_string()),
        1 => prop::sample::select(vec!['\n', '\r', '\t', '\u{0}', '\u{7f}', '\u{85}', '\u{202e}', '\u{2066}', '\u{200b}', '\u{feff}', '\u{e0041}', '\u{1b}'])
            .prop_map(String::from),
        1 => any::<char>().prop_map(String::from),
    ];
    prop::collection::vec(piece, 0..20).prop_map(|pieces| pieces.concat())
}

fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{2028}' | '\u{2029}' | '\u{061c}' | '\u{200e}' | '\u{200f}'
        )
        || ('\u{202a}'..='\u{202e}').contains(&c)
        || ('\u{2066}'..='\u{2069}').contains(&c)
        || ('\u{200b}'..='\u{200d}').contains(&c)
        || ('\u{e0000}'..='\u{e007f}').contains(&c)
        || c == '\u{feff}'
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
