//! The report format: what is written reads back, and what cannot be read is refused.

use proptest::prelude::*;

use super::{MAX_REPORT_BYTES, Report, ReportFault, ReportKey, decode, encode, read};

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

fn key() -> ReportKey {
    ReportKey::from_words(0x0123_4567_89ab_cdef, 42)
}

#[test]
fn a_report_is_one_line_of_tagged_entries() {
    let report = Report {
        key: key().as_str().to_owned(),
        env: pairs(&[("EDITOR", "vim"), ("PATH", "/bin:/usr/bin")]),
        aliases: pairs(&[("ll", "ls -l")]),
    };
    assert_eq!(
        encode(&report),
        "v2;k=0123456789abcdef000000000000002a;e=EDITOR=vim;e=PATH=/bin%3A/usr/bin;a=ll=ls%20-l"
    );
    assert_eq!(decode(&encode(&report)), Ok(report.clone()));
    assert_eq!(read(&encode(&report), &key()), Ok(report));
}

#[test]
fn a_report_without_the_shells_key_is_refused() {
    let other = ReportKey::from_words(1, 2);
    let signed = |key: &str| {
        encode(&Report {
            key: key.to_owned(),
            env: pairs(&[("LD_PRELOAD", "/tmp/evil.so")]),
            aliases: pairs(&[("ls", "curl x | sh")]),
        })
    };
    // (what, payload)
    let cases = [
        ("another shell's key", signed(other.as_str())),
        ("no key", signed("")),
        ("no key entry at all", "v2;e=LD_PRELOAD=x".to_owned()),
        ("the key cut short", signed(&key().as_str()[..31])),
    ];
    for (what, payload) in cases {
        assert_eq!(read(&payload, &key()), Err(ReportFault::Key), "{what}");
    }
}

#[test]
fn a_key_reads_back_only_as_32_hex_digits() {
    assert_eq!(ReportKey::parse(key().as_str()), Some(key()));
    assert_eq!(
        ReportKey::parse("0123456789ABCDEF000000000000002A"),
        Some(key())
    );
    for bad in ["", "abc", &"g".repeat(32), &"a".repeat(33)] {
        assert_eq!(ReportKey::parse(bad), None, "{bad:?}");
    }
    assert!(
        !format!("{:?}", key()).contains("0123"),
        "not shown in output"
    );
}

#[test]
fn what_cannot_be_read_is_refused() {
    let big = format!("v2;e=A={}", "x".repeat(MAX_REPORT_BYTES));
    // (what, payload, the fault)
    let cases = [
        (
            "a version this build does not read",
            "v3;e=A=1",
            ReportFault::Version,
        ),
        (
            "the first version, which had no key",
            "v1;e=A=1",
            ReportFault::Version,
        ),
        ("no version", "", ReportFault::Version),
        (
            "an entry with no value part",
            "v2;e=A",
            ReportFault::Malformed,
        ),
        ("an entry with no tag", "v2;junk", ReportFault::Malformed),
        ("a broken escape", "v2;e=A=%zz", ReportFault::Malformed),
        ("an empty name", "v2;e==1", ReportFault::Malformed),
        (
            "a value that is not UTF-8",
            "v2;e=A=%FF",
            ReportFault::Malformed,
        ),
        (
            "a variable name with a dash",
            "v2;e=A-B=1",
            ReportFault::Malformed,
        ),
        (
            "a variable name starting with a digit",
            "v2;e=1A=1",
            ReportFault::Malformed,
        ),
        (
            "a variable name with a space",
            "v2;e=A%20B=1",
            ReportFault::Malformed,
        ),
        (
            "a NUL in a variable name",
            "v2;e=A%00=1",
            ReportFault::Malformed,
        ),
        ("a NUL in a value", "v2;e=A=x%00y", ReportFault::Malformed),
        (
            "a NUL in an alias name",
            "v2;a=l%00l=ls",
            ReportFault::Malformed,
        ),
        (
            "a NUL in an alias value",
            "v2;a=ll=ls%00",
            ReportFault::Malformed,
        ),
        (
            "an alias name with =",
            "v2;a=a%3Db=x",
            ReportFault::Malformed,
        ),
        (
            "an alias name with /",
            "v2;a=bin/ls=x",
            ReportFault::Malformed,
        ),
        (
            "an alias name with a space",
            "v2;a=a%20b=x",
            ReportFault::Malformed,
        ),
        (
            "an alias name with a tab",
            "v2;a=a%09b=x",
            ReportFault::Malformed,
        ),
        ("an empty alias name", "v2;a==x", ReportFault::Malformed),
        ("one over the cap", big.as_str(), ReportFault::TooLarge),
    ];
    for (what, payload, fault) in cases {
        assert_eq!(decode(payload), Err(fault), "{what}");
    }
}

#[test]
fn names_a_shell_gives_are_read() {
    let got =
        decode("v2;e=_x1=1;e=Path_2=2;a=g.co=git%20checkout;a=-x=y;a=%2B%2B=z").expect("reads");
    assert_eq!(got.env, pairs(&[("_x1", "1"), ("Path_2", "2")]));
    assert_eq!(
        got.aliases,
        pairs(&[("g.co", "git checkout"), ("-x", "y"), ("++", "z")])
    );
}

#[test]
fn a_tag_from_a_newer_script_is_skipped() {
    let got = decode("v2;f=run=abc123;e=A=1;;").expect("reads");
    assert_eq!(got.env, pairs(&[("A", "1")]));
}

proptest! {
    #[test]
    fn whatever_a_shell_holds_reads_back(
        env in proptest::collection::vec(("[A-Za-z_][A-Za-z0-9_]{0,12}", "[^\0]{0,30}"), 0..8),
        aliases in proptest::collection::vec(("[a-z]{1,6}", "[^\0]{0,30}"), 0..5),
    ) {
        let report = Report { key: key().as_str().to_owned(), env, aliases };
        prop_assert_eq!(read(&encode(&report), &key()), Ok(report));
    }
}
