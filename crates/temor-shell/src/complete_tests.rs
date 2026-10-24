//! The completion answer: what a script writes reads back, what cannot be read is refused, and
//! a word takes the place of the line's end.

use super::{Answer, AnswerFault, Completions, MAX_ANSWER_BYTES, decode};

fn completed(at: usize, words: &[&str]) -> Completions {
    Completions {
        at,
        words: words.iter().map(|w| (*w).to_owned()).collect(),
    }
}

#[test]
fn an_answer_is_ready_or_the_words_for_the_lines_end() {
    // (what, payload, the answer)
    let cases = [
        ("the shell answers the key", "v1;ready", Answer::Ready),
        (
            "words, escaped",
            "v1;at=4;c=checkout;c=cherry-pick;c=my%20file",
            Answer::Completed(completed(4, &["checkout", "cherry-pick", "my file"])),
        ),
        (
            "nothing to complete",
            "v1;at=0",
            Answer::Completed(completed(0, &[])),
        ),
        (
            "a word said twice, an empty one and a tag a newer script adds",
            "v1;at=2;c=ab;c=;c=ab;x=1;c=ac",
            Answer::Completed(completed(2, &["ab", "ac"])),
        ),
    ];
    for (what, payload, want) in cases {
        assert_eq!(decode(payload), Ok(want), "{what}");
    }
}

#[test]
fn what_cannot_be_read_is_refused() {
    let big = format!("v1;at=0;c={}", "x".repeat(MAX_ANSWER_BYTES));
    // (what, payload, the fault)
    let cases = [
        (
            "a version this build does not read",
            "v2;ready",
            AnswerFault::Version,
        ),
        ("no version", "", AnswerFault::Version),
        ("no place", "v1", AnswerFault::Malformed),
        (
            "a place that is not a number",
            "v1;at=x;c=a",
            AnswerFault::Malformed,
        ),
        ("a broken escape", "v1;at=0;c=%zz", AnswerFault::Malformed),
        ("too large", big.as_str(), AnswerFault::TooLarge),
    ];
    for (what, payload, fault) in cases {
        assert_eq!(decode(payload), Err(fault), "{what}");
    }
}

#[test]
fn a_word_takes_the_place_of_the_lines_end_counted_in_characters() {
    let c = completed(4, &[]);
    assert_eq!(
        c.applied("git ch", "checkout").as_deref(),
        Some("git checkout")
    );
    assert_eq!(
        completed(6, &[]).applied("cat é/f", "fichier").as_deref(),
        Some("cat é/fichier"),
        "a place counts characters, not bytes"
    );
    assert_eq!(completed(4, &[]).applied("ls ", "x"), None, "past the line");
    assert_eq!(
        completed(3, &[]).applied("ls ", "src/").as_deref(),
        Some("ls src/")
    );
}

#[test]
fn the_words_share_what_they_all_start_with() {
    // (words, what they share)
    let cases: [(&[&str], &str); 5] = [
        (&[], ""),
        (&["checkout"], "checkout"),
        (&["checkout", "cherry-pick"], "che"),
        (&["src/", "src/lib.rs"], "src/"),
        (&["éa", "éb"], "é"),
    ];
    for (words, shared) in cases {
        assert_eq!(completed(0, words).shared(), shared, "{words:?}");
    }
}
