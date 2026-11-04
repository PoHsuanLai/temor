//! A suggestion's place in the line, the last word of a line, and nushell's output read back.

use super::{NushellFault, SourceId, Suggestion, common_start, last_word_start, read_nushell};

fn word(word: &str, from: usize, to: usize, space: bool) -> Suggestion {
    Suggestion {
        word: word.to_owned(),
        from,
        to,
        space,
        detail: None,
    }
}

#[test]
fn a_suggestion_takes_the_place_of_the_characters_it_names() {
    // (what, the suggestion, the line, the line made)
    let cases = [
        (
            "the last word",
            word("checkout", 4, 6, true),
            "git ch",
            "git checkout ",
        ),
        (
            "a folder goes on",
            word("src/", 3, 5, false),
            "ls sr",
            "ls src/",
        ),
        (
            "a whole command for what was typed",
            word("git reset --soft HEAD~1", 0, 12, false),
            "undo commit.",
            "git reset --soft HEAD~1",
        ),
        (
            "characters, not bytes",
            word("fichier", 6, 8, false),
            "cat é/fi",
            "cat é/fichier",
        ),
        (
            "what follows the place stays",
            word("b", 0, 1, false),
            "a rest",
            "b rest",
        ),
    ];
    for (what, suggestion, line, want) in cases {
        assert_eq!(suggestion.applied(line).as_deref(), Some(want), "{what}");
    }
    assert_eq!(word("x", 0, 9, true).applied("ls"), None, "past the line");
    assert_eq!(word("x", 2, 1, true).applied("ls"), None, "backwards");
}

#[test]
fn the_last_word_ends_at_a_space_outside_quotes() {
    // (the line, where the last word starts)
    let cases = [
        ("", 0),
        ("ls", 0),
        ("ls ", 3),
        ("git ch", 4),
        ("ls \"my d", 3),
        ("ls `my d", 3),
        ("echo 'a b' c", 11),
        ("echo 'a b'", 5),
        ("é é", 2),
    ];
    for (line, want) in cases {
        assert_eq!(last_word_start(line), want, "{line:?}");
    }
}

#[test]
fn nushells_completions_replace_the_last_word() {
    let out = r#"{"completions": ["mine/", "`my dir/`", "mix.nu"]}"#;
    let got = read_nushell(out, "ls m").expect("read");
    assert_eq!(
        got,
        vec![
            word("mine/", 3, 4, false),
            word("`my dir/`", 3, 4, false),
            word("mix.nu", 3, 4, true),
        ]
    );
    let quoted = read_nushell(r#"{"completions": ["`my dir/`"]}"#, "ls \"my d").expect("read");
    assert_eq!(
        quoted,
        vec![word("`my dir/`", 3, 8, false)],
        "a quoted word is replaced whole"
    );
    assert_eq!(read_nushell(r#"{"completions": []}"#, "git ch"), Ok(vec![]));
}

#[test]
fn what_nushell_did_not_print_as_json_is_refused() {
    for out in [
        "",
        "error: nope",
        r#"{"other": 1}"#,
        r#"{"completions": "x"}"#,
    ] {
        assert_eq!(
            read_nushell(out, "ls"),
            Err(NushellFault::NotJson),
            "{out:?}"
        );
    }
}

#[test]
fn sources_are_named_by_their_id() {
    assert_eq!(SourceId("nushell"), SourceId("nushell"));
    assert_ne!(SourceId("nushell"), SourceId("shell"));
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
        assert_eq!(common_start(words), shared, "{words:?}");
    }
}
