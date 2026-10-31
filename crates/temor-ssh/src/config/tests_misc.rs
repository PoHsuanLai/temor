//! Lexer and glob matching.

use super::glob::glob_match;
use super::lex::lex_line;

#[test]
fn lexer_handles_quotes_escapes_and_comments() {
    let line = lex_line("  Match \"a b\" \"c\\\\d\\\"e\" x\"y z\"")
        .ok()
        .flatten();
    let line = line.expect("a keyword line");
    assert_eq!(line.keyword, "match");
    assert_eq!(line.args, vec!["a b", "c\\d\"e", "xy z"]);
    assert!(lex_line("# comment").is_ok_and(|l| l.is_none()));
    assert!(lex_line("   ").is_ok_and(|l| l.is_none()));
    assert!(lex_line("User \"open").is_err());
}

#[test]
fn glob_matching_is_case_insensitive_with_wildcards() {
    assert!(glob_match("web*", "WEB-1"));
    assert!(glob_match("w?b", "web"));
    assert!(glob_match("*", ""));
    assert!(glob_match("a*b*c", "axxbyyc"));
    assert!(!glob_match("a*b", "ac"));
    assert!(!glob_match("w?b", "wb"));
    assert!(glob_match("*.conf", "x.conf"));
}
