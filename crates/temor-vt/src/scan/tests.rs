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

    /// The cut offsets over `chunks`, as absolute offsets into their concatenation.
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

    /// The offset of the first `needle` in `text`.
    fn find(text: &[u8], needle: &[u8]) -> usize {
        text.windows(needle.len())
            .position(|w| w == needle)
            .expect("needle present")
    }

    #[test]
    fn a_bel_terminated_mark_is_cut_after_the_bel() {
        let text = b"ab\x1b]133;A\x07cd";
        assert_eq!(cuts(&[text]), vec![find(text, b"cd")]);
    }

    #[test]
    fn an_st_terminated_mark_is_cut_after_its_escape() {
        // libghostty reports the mark when it reads the ESC of ST, so the ESC stays with the mark.
        // Regression: a cut before the ESC sent each bash mark's event after its anchor.
        let text = b"ab\x1b]133;D;0\x1b\\cd";
        assert_eq!(cuts(&[text]), vec![find(text, b"\\cd")]);
        // A chunk that starts with the ESC cuts right after it, never before it.
        let mut scanner = MarkCuts::new();
        assert!(scanner.feed(b"ab\x1b]133;D;0").is_empty());
        assert_eq!(scanner.feed(b"\x1b"), vec![1]);
    }

    #[test]
    fn an_escape_inside_a_mark_ends_it_and_starts_the_next_sequence() {
        // ESC [0m ends the first mark (no BEL): the cut falls after the ESC, before the `[`. The
        // second mark is cut after its BEL.
        let text = b"\x1b]133;A\x1b[0mx\x1b]133;B\x07";
        assert_eq!(
            cuts(&[text]),
            vec![find(text, b"[0m"), find(text, b"\x1b]133;B") + 8]
        );
    }

    #[test]
    fn marks_inside_dcs_payloads_are_not_marks() {
        assert!(cuts(&[b"\x1bP133;A\x1b\\"]).is_empty());
        assert!(cuts(&[b"\x1bP133;A\x9c133;B\x07\x1b\\"]).is_empty());
        assert!(
            cuts(&[b"\x1bP\x9d133;A\x9c"]).is_empty(),
            "C1 bytes are payload in DCS"
        );
    }

    #[test]
    fn marks_inside_apc_payloads_are_not_marks_but_an_escape_inside_one_starts_a_mark() {
        assert!(cuts(&[b"\x1b_133;A\x07\x1b\\"]).is_empty());
        // ESC aborts the APC, and the OSC it starts is a real mark (as libghostty reads it).
        let text = b"\x1b_\x1b]133;A\x07\x1b\\";
        assert_eq!(cuts(&[text]), vec![find(text, b"\x1b]133;A\x07") + 8]);
    }

    #[test]
    fn c1_forms_of_osc_and_st_are_recognised() {
        assert_eq!(cuts(&[b"\x9d133;A\x9c"]), vec![7]);
        assert_eq!(cuts(&[b"\x9d133;A\x07"]), vec![7]);
    }

    #[test]
    fn other_osc_sequences_and_plain_text_make_no_cuts() {
        assert!(cuts(&[b"\x1b]0;133;A\x07"]).is_empty());
        assert!(cuts(&[b"\x1b]1337;File=x\x07 133;A\x07"]).is_empty());
        assert!(cuts(&[b"133;A\x07 \x1b[0m plain text"]).is_empty());
    }

    #[test]
    fn several_marks_in_one_chunk_each_cut() {
        assert_eq!(cuts(&[b"\x1b]133;A\x07\x1b]133;B\x07"]), vec![8, 16]);
    }

    #[test]
    fn marks_split_at_every_offset_are_found_once() {
        let whole = b"x\x1b]133;C\x07y";
        let expected = vec![find(whole, b"y")];
        for split in 0..=whole.len() {
            let (a, b) = whole.split_at(split);
            assert_eq!(cuts(&[a, b]), expected, "split at {split}");
        }
        let st = b"x\x1b]133;C\x1b\\y";
        for split in 0..=st.len() {
            let (a, b) = st.split_at(split);
            assert_eq!(
                cuts(&[a, b]),
                vec![find(st, b"\x1b\\") + 1],
                "st split at {split}"
            );
        }
    }
}
