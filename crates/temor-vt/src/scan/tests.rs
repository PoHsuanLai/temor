use super::{InlineFileScanner, Scanned};
use crate::{ByteCount, Truncation};

/// What a run of chunks produced: the bytes libghostty gets and each file's payload.
#[derive(Debug, Default, PartialEq, Eq)]
struct Outcome {
    passed: Vec<u8>,
    files: Vec<(Vec<u8>, Truncation)>,
}

fn run(chunks: &[&[u8]], max: usize) -> Outcome {
    let mut scanner = InlineFileScanner::new(ByteCount(max));
    let mut outcome = Outcome::default();
    for chunk in chunks {
        scanner.feed(chunk, |s| match s {
            Scanned::Bytes(b) => outcome.passed.extend_from_slice(b),
            Scanned::File {
                payload,
                truncation,
            } => outcome.files.push((payload, truncation)),
        });
    }
    outcome
}

fn file(payload: &str) -> (Vec<u8>, Truncation) {
    (payload.as_bytes().to_vec(), Truncation::Complete)
}

/// (name, input, bytes passed on, payloads taken out)
const CASES: &[(&str, &str, &str, &[&str])] = &[
    ("plain text passes", "hello\r\nworld", "hello\r\nworld", &[]),
    (
        "bel terminated file",
        "a\x1b]1337;File=name=eA==;size=1:QQ==\x07b",
        "ab",
        &["1337;File=name=eA==;size=1:QQ=="],
    ),
    (
        "st terminated file",
        "\x1b]1337;File=:QQ==\x1b\\z",
        "z",
        &["1337;File=:QQ=="],
    ),
    (
        "other osc passes",
        "\x1b]133;A\x07$ ",
        "\x1b]133;A\x07$ ",
        &[],
    ),
    (
        "other 1337 key passes",
        "\x1b]1337;CurrentDir=/tmp\x07",
        "\x1b]1337;CurrentDir=/tmp\x07",
        &[],
    ),
    (
        "csi passes",
        "\x1b[1;31mred\x1b[0m",
        "\x1b[1;31mred\x1b[0m",
        &[],
    ),
    (
        "double escape",
        "\x1b\x1b]1337;File=:x\x07",
        "\x1b",
        &["1337;File=:x"],
    ),
    ("can cancels", "\x1b]1337;File=:abc\x18after", "after", &[]),
    (
        "escape cancels and restarts",
        "\x1b]1337;File=:abc\x1b]1337;File=:d\x07",
        "",
        &["1337;File=:d"],
    ),
    (
        "escape cancels into csi",
        "\x1b]1337;File=:abc\x1b[2J",
        "\x1b[2J",
        &[],
    ),
    (
        "two files",
        "\x1b]1337;File=:a\x07-\x1b]1337;File=:b\x07",
        "-",
        &["1337;File=:a", "1337;File=:b"],
    ),
];

#[test]
fn whole_chunks_split_files_from_text() {
    for (name, input, passed, files) in CASES {
        let got = run(&[input.as_bytes()], 1 << 20);
        let want = Outcome {
            passed: passed.as_bytes().to_vec(),
            files: files.iter().map(|f| file(f)).collect(),
        };
        assert_eq!(got, want, "case {name}");
    }
}

#[test]
fn every_two_way_split_matches_the_whole() {
    for (name, input, _, _) in CASES {
        let whole = run(&[input.as_bytes()], 1 << 20);
        for at in 0..=input.len() {
            let (a, b) = input.as_bytes().split_at(at);
            assert_eq!(run(&[a, b], 1 << 20), whole, "case {name}, split at {at}");
        }
    }
}

#[test]
fn byte_at_a_time_matches_the_whole() {
    for (name, input, _, _) in CASES {
        let chunks: Vec<&[u8]> = input.as_bytes().chunks(1).collect();
        assert_eq!(
            run(&chunks, 1 << 20),
            run(&[input.as_bytes()], 1 << 20),
            "case {name}"
        );
    }
}

#[test]
fn a_held_prefix_at_the_end_is_released_by_the_next_chunk() {
    let mut scanner = InlineFileScanner::new(ByteCount(64));
    let mut first = Vec::new();
    scanner.feed(b"ok\x1b]13", |s| first.push(format!("{s:?}")));
    assert_eq!(first, [format!("{:?}", Scanned::Bytes(b"ok"))]);
    let got = run(&[b"ok\x1b]13", b"3;A\x07"], 64);
    assert_eq!(got.passed, b"ok\x1b]133;A\x07");
}

#[test]
fn payload_over_the_limit_is_cut_and_marked() {
    let got = run(&[b"\x1b]1337;File=:0123456789\x07"], 4);
    assert_eq!(
        got.files,
        [(b"1337;File=:012".to_vec(), Truncation::Truncated)]
    );
    assert_eq!(got.passed, b"");
}

#[test]
fn cancelling_drops_the_held_payload() {
    for cancel in [&b"\x18"[..], b"\x1a", b"\x1bx"] {
        let mut scanner = InlineFileScanner::new(ByteCount(1 << 20));
        scanner.feed(b"\x1b]1337;File=:0123456789", |_| {});
        assert!(scanner.held_payload() > 0);
        scanner.feed(cancel, |_| {});
        assert_eq!(scanner.held_payload(), 0, "cancel {cancel:?}");
    }
}

#[cfg(feature = "ghostty")]
mod marks {
    use super::super::MarkCuts;

    /// The cut ends over `chunks`, as absolute offsets into their concatenation.
    fn cuts(chunks: &[&[u8]]) -> Vec<usize> {
        let mut scanner = MarkCuts::new();
        let mut at = 0;
        let mut out = Vec::new();
        for chunk in chunks {
            out.extend(scanner.feed(chunk).into_iter().map(|end| at + end));
            at += chunk.len();
        }
        out
    }

    #[test]
    fn a_bel_or_st_terminated_mark_ends_after_its_terminator() {
        assert_eq!(cuts(&[b"ab\x1b]133;A\x07cd"]), vec![2 + 8]);
        assert_eq!(cuts(&[b"ab\x1b]133;D;0\x1b\\cd"]), vec![2 + 11]);
    }

    #[test]
    fn marks_split_anywhere_are_found_once_complete() {
        let whole = b"x\x1b]133;C\x1b\\y";
        for split in 0..=whole.len() {
            let (a, b) = whole.split_at(split);
            assert_eq!(cuts(&[a, b]), vec![whole.len() - 1], "split at {split}");
        }
    }

    #[test]
    fn other_osc_sequences_and_text_are_not_marks() {
        assert!(cuts(&[b"\x1b]0;133;A\x07"]).is_empty());
        assert!(cuts(&[b"\x1b]1337;File=x\x07 133;A\x07"]).is_empty());
        assert!(cuts(&[b"133;A\x07 \x1b[0m"]).is_empty());
    }

    #[test]
    fn several_marks_in_one_chunk_each_cut() {
        assert_eq!(cuts(&[b"\x1b]133;A\x07\x1b]133;B\x07"]), vec![8, 16]);
    }
}
