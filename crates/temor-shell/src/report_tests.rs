//! The report format: what is written reads back, and what cannot be read is refused.

use proptest::prelude::*;

use super::{MAX_REPORT_BYTES, Report, ReportFault, decode, encode};

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn a_report_is_one_line_of_tagged_entries() {
    let report = Report {
        env: pairs(&[("EDITOR", "vim"), ("PATH", "/bin:/usr/bin")]),
        aliases: pairs(&[("ll", "ls -l")]),
    };
    assert_eq!(
        encode(&report),
        "v1;e=EDITOR=vim;e=PATH=/bin%3A/usr/bin;a=ll=ls%20-l"
    );
    assert_eq!(decode(&encode(&report)), Ok(report));
}

#[test]
fn what_cannot_be_read_is_refused() {
    let big = format!("v1;e=A={}", "x".repeat(MAX_REPORT_BYTES));
    // (what, payload, the fault)
    let cases = [
        (
            "a version this build does not read",
            "v2;e=A=1",
            ReportFault::Version,
        ),
        ("no version", "", ReportFault::Version),
        (
            "an entry with no value part",
            "v1;e=A",
            ReportFault::Malformed,
        ),
        ("an entry with no tag", "v1;junk", ReportFault::Malformed),
        ("a broken escape", "v1;e=A=%zz", ReportFault::Malformed),
        ("an empty name", "v1;e==1", ReportFault::Malformed),
        (
            "a value that is not UTF-8",
            "v1;e=A=%FF",
            ReportFault::Malformed,
        ),
        ("one over the cap", big.as_str(), ReportFault::TooLarge),
    ];
    for (what, payload, fault) in cases {
        assert_eq!(decode(payload), Err(fault), "{what}");
    }
}

#[test]
fn a_tag_from_a_newer_script_is_skipped() {
    let got = decode("v1;f=run=abc123;e=A=1;;").expect("reads");
    assert_eq!(got.env, pairs(&[("A", "1")]));
}

proptest! {
    #[test]
    fn whatever_a_shell_holds_reads_back(
        env in proptest::collection::vec(("[A-Za-z_][A-Za-z0-9_]{0,12}", ".{0,30}"), 0..8),
        aliases in proptest::collection::vec(("[a-z]{1,6}", ".{0,30}"), 0..5),
    ) {
        let report = Report { env, aliases };
        prop_assert_eq!(decode(&encode(&report)), Ok(report));
    }
}
