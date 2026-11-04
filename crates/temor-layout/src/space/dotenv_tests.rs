//! The dotenv reader's rules, one row each (see the module docs for the rules).

use super::{DotenvFault, parse_dotenv};

fn vars(text: &str) -> Vec<(String, String)> {
    parse_dotenv(text)
        .vars
        .into_iter()
        .map(|(k, v)| (k.as_str().to_owned(), v.0))
        .collect()
}

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn values_follow_the_quoting_rules() {
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)] // a table of named rows
    let cases: &[(&str, &str, &[(&str, &str)])] = &[
        ("plain", "A=b", &[("A", "b")]),
        ("empty value", "A=", &[("A", "")]),
        ("blanks trimmed when unquoted", "A=  b c  ", &[("A", "b c")]),
        ("comment after a blank", "A=b # note", &[("A", "b")]),
        ("hash without a blank is text", "A=b#c", &[("A", "b#c")]),
        ("a hash first is a comment", "A= #x", &[("A", "")]),
        ("quotes inside unquoted are text", "A=it's \"x\"", &[("A", "it's \"x\"")]),
        ("single quotes are literal", "A='a \\n $B #c'", &[("A", "a \\n $B #c")]),
        ("single quotes keep blanks", "A='  x  '", &[("A", "  x  ")]),
        ("double quotes keep blanks", "A=\"  x  \"", &[("A", "  x  ")]),
        ("double quote escapes", "A=\"l1\\nl2\\t\\\\ \\\" \\$\"", &[("A", "l1\nl2\t\\ \" $")]),
        ("a comment after a closing quote", "A=\"x\" # y", &[("A", "x")]),
        ("no expansion", "A=$HOME\nB=${X}\nC=$(id)\nD=\"$HOME\"", &[("A", "$HOME"), ("B", "${X}"), ("C", "$(id)"), ("D", "$HOME")]),
        ("export prefix", "export A=1\n  export   B=2", &[("A", "1"), ("B", "2")]),
        ("a name that only starts like export", "exported=1", &[("exported", "1")]),
        ("comments, blanks and CRLF", "# c\r\n\r\n  # d\r\nA=1\r\n", &[("A", "1")]),
        ("a byte-order mark", "\u{feff}A=1", &[("A", "1")]),
        ("last value wins at the first place", "A=1\nB=2\nA=3", &[("A", "3"), ("B", "2")]),
    ];
    for (name, text, want) in cases {
        let parsed = parse_dotenv(text);
        assert_eq!(vars(text), pairs(want), "{name}");
        assert_eq!(parsed.faults, vec![], "{name}");
    }
}

#[test]
fn bad_lines_are_reported_and_skipped_and_the_rest_applies() {
    #[rustfmt::skip]
    let cases: &[(&str, &str, DotenvFault)] = &[
        ("no equals", "JUST_A_WORD", DotenvFault::NoEquals),
        ("export alone", "export A", DotenvFault::NoEquals),
        ("space before the equals", "A =1", DotenvFault::BadKey),
        ("empty name", "=1", DotenvFault::BadKey),
        ("digit first", "1A=1", DotenvFault::BadKey),
        ("dash in the name", "A-B=1", DotenvFault::BadKey),
        ("single quote never closed", "A='x", DotenvFault::Unterminated),
        ("double quote never closed", "A=\"x", DotenvFault::Unterminated),
        ("escaped closing quote never closed", "A=\"x\\\"", DotenvFault::Unterminated),
        ("text after a closing quote", "A='x' y", DotenvFault::TrailingText),
        ("text after a double quote", "A=\"x\"y", DotenvFault::TrailingText),
        ("unknown escape", "A=\"\\q\"", DotenvFault::BadEscape),
        ("backslash at the end of a double quote", "A=\"x\\", DotenvFault::BadEscape),
        ("a control character", "A=x\u{7}y", DotenvFault::ControlCharacter),
        ("a lone carriage return", "A=x\ry", DotenvFault::ControlCharacter),
    ];
    for (name, bad, fault) in cases {
        let parsed = parse_dotenv(&format!("FIRST=1\n{bad}\nLAST=2\n"));
        assert_eq!(parsed.faults, vec![(2, *fault)], "{name}");
        assert_eq!(
            vars(&format!("FIRST=1\n{bad}\nLAST=2")),
            pairs(&[("FIRST", "1"), ("LAST", "2")]),
            "{name}"
        );
    }
}
