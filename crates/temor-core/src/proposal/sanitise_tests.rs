use super::sanitise::{CharClass, Sanitised, sanitise};
use CharClass::{Bidi, Control, EscapeSequence, Invisible, Newline, Tab};

#[test]
fn sanitise_removes_each_unsafe_class_and_reports_it() {
    #[rustfmt::skip]
    const CASES: &[(&str, &str, &str, &[CharClass])] = &[
        ("clean text is unchanged",          "ls -la ~/src",              "ls -la ~/src", &[]),
        ("unicode text is kept",             "echo héllo 世界 🎉",         "echo héllo 世界 🎉", &[]),
        ("a trailing newline",               "ls\n",                      "ls", &[Newline]),
        ("a second command after LF",        "ls\nrm -rf ~",              "lsrm -rf ~", &[Newline]),
        ("CR LF",                            "ls\r\n",                    "ls", &[Newline]),
        ("vertical tab and form feed",       "a\u{0b}b\u{0c}c",           "abc", &[Newline]),
        ("NEL",                              "a\u{85}b",                  "ab", &[Newline]),
        ("line and paragraph separators",    "a\u{2028}b\u{2029}c",       "abc", &[Newline]),
        ("a tab",                            "a\tb",                      "ab", &[Tab]),
        ("SGR colour",                       "\u{1b}[31mred\u{1b}[0m",    "red", &[EscapeSequence]),
        ("bracketed paste end injection",    "ls\u{1b}[201~; rm -rf ~",   "ls; rm -rf ~", &[EscapeSequence]),
        ("CSI with private parameters",      "a\u{1b}[?2004lb",           "ab", &[EscapeSequence]),
        ("OSC title ended by BEL",           "\u{1b}]0;pwned\u{07}ls",    "ls", &[EscapeSequence]),
        ("OSC 52 clipboard ended by ST",     "\u{1b}]52;c;aGk=\u{1b}\\ls", "ls", &[EscapeSequence]),
        ("an unterminated OSC eats the rest", "ls\u{1b}]8;;http://x",     "ls", &[EscapeSequence]),
        ("DCS string",                       "\u{1b}Pq#0\u{1b}\\ok",      "ok", &[EscapeSequence]),
        ("APC string",                       "\u{1b}_Gf=24\u{1b}\\ok",    "ok", &[EscapeSequence]),
        ("a two-character escape",           "a\u{1b}cb",                 "ab", &[EscapeSequence]),
        ("an escape with intermediates",     "a\u{1b}(Bb",                "ab", &[EscapeSequence]),
        ("a lone escape at the end",         "ls\u{1b}",                  "ls", &[EscapeSequence]),
        ("8-bit CSI",                        "a\u{9b}31mb",               "ab", &[EscapeSequence]),
        ("8-bit OSC ended by 8-bit ST",      "a\u{9d}0;x\u{9c}b",         "ab", &[EscapeSequence]),
        ("NUL and BEL",                      "a\u{0}b\u{7}c",             "abc", &[Control]),
        ("backspace overwriting",            "rm -rf ~\u{8}\u{8}\u{8}tmp", "rm -rf ~tmp", &[Control]),
        ("DEL",                              "a\u{7f}b",                  "ab", &[Control]),
        ("a lone C1 control",                "a\u{80}b\u{9c}c",           "abc", &[Control]),
        ("right-to-left override",           "ls \u{202e}txt.exe",        "ls txt.exe", &[Bidi]),
        ("bidi embeddings and pop",          "\u{202a}a\u{202b}b\u{202c}", "ab", &[Bidi]),
        ("bidi isolates",                    "\u{2066}a\u{2067}b\u{2068}c\u{2069}", "abc", &[Bidi]),
        ("marks LRM RLM ALM",                "a\u{200e}b\u{200f}c\u{61c}d", "abcd", &[Bidi]),
        ("zero-width space and joiners",     "r\u{200b}m\u{200c}\u{200d}", "rm", &[Invisible]),
        ("BOM and word joiner",              "\u{feff}ls\u{2060}",        "ls", &[Invisible]),
        ("soft hyphen",                      "c\u{ad}at",                 "cat", &[Invisible]),
        ("tag characters smuggling text",    "ls\u{e0072}\u{e006d}",      "ls", &[Invisible]),
        ("variation selectors",              "a\u{fe0f}b\u{e0100}",       "ab", &[Invisible]),
        ("classes reported once, in order",  "\u{202e}a\nb\u{1b}[0m\nc\u{202e}\t", "abc", &[Bidi, Newline, EscapeSequence, Tab]),
        ("an escape string swallows newlines inside it", "\u{1b}]0;a\nb\u{07}ok", "ok", &[EscapeSequence]),
    ];
    for (name, raw, want, removed) in CASES {
        let (line, report) = sanitise(raw);
        assert_eq!(line.as_str(), *want, "{name}: text");
        let expected = if removed.is_empty() {
            Sanitised::Unchanged
        } else {
            Sanitised::Changed {
                removed: removed.to_vec(),
            }
        };
        assert_eq!(report, expected, "{name}: report");
    }
}

#[test]
fn every_c0_and_c1_control_is_removed() {
    for code in (0u32..0x20).chain(0x7f..0xa0) {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        let (line, report) = sanitise(&format!("a{c}b"));
        assert!(
            !line.as_str().contains(c) && matches!(report, Sanitised::Changed { .. }),
            "U+{code:04X} left {:?}",
            line.as_str()
        );
    }
}

#[test]
fn a_stored_line_is_sanitised_again_as_it_is_read() {
    let line: super::SanitisedLine = serde_json::from_str("\"ls\\u001b[201~\\nrm\"").unwrap();
    assert_eq!(line.as_str(), "lsrm");
    assert_eq!(serde_json::to_string(&line).unwrap(), "\"lsrm\"");
}
