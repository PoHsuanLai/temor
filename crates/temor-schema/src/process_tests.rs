use super::{CommandLine, MAX_BYTES, MAX_TOKENS};

/// A row: its name, the bytes of `/proc/<pid>/cmdline`, and the tokens wanted (none: no line).
type Case<'a> = (&'a str, &'a [u8], Option<Vec<&'a str>>);

#[test]
fn a_cmdline_is_split_at_nuls_and_bounded() {
    let many = "a\0".repeat(MAX_TOKENS + 1);
    let just_enough = "a\0".repeat(MAX_TOKENS);
    let huge = vec![b'a'; MAX_BYTES + 1];
    let cases: Vec<Case> = vec![
        (
            "two words",
            b"claude\0--resume\0",
            Some(vec!["claude", "--resume"]),
        ),
        ("no final NUL", b"claude\0x", Some(vec!["claude", "x"])),
        ("an empty argument", b"a\0\0b\0", Some(vec!["a", "", "b"])),
        ("bad UTF-8 is replaced", b"a\xff\0", Some(vec!["a\u{fffd}"])),
        ("empty", b"", None),
        (
            "at the token bound",
            just_enough.as_bytes(),
            Some(vec!["a"; MAX_TOKENS]),
        ),
        ("past the token bound", many.as_bytes(), None),
        ("past the byte bound", &huge, None),
    ];
    for (name, bytes, want) in cases {
        let got = CommandLine::from_cmdline(bytes);
        assert_eq!(
            got.as_ref().map(|c| c.tokens().to_vec()),
            want.map(|w| { w.into_iter().map(str::to_owned).collect::<Vec<_>>() }),
            "{name}"
        );
    }
}

#[test]
fn debug_never_prints_the_arguments() {
    let line = CommandLine::from_cmdline(b"tool\0--token=secret\0").expect("a line");
    assert_eq!(format!("{line:?}"), "CommandLine(2 tokens)");
}
