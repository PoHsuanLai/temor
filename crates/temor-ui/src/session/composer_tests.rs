//! The composer's history from plain lists: no session, no shell.

use super::*;

#[test]
fn the_history_is_newest_first_each_command_once_where_it_last_ran() {
    let commands = ["ls", "git status", "ls", "", "make  ", "make"];
    assert_eq!(
        newest_first(commands.into_iter()),
        ["make", "ls", "git status"]
    );
}

#[test]
fn stepping_goes_back_to_the_oldest_and_forward_past_the_newest_to_what_was_typed() {
    use HistoryStep::{Newer, Older};
    assert_eq!(stepped(None, Older, 3), Some(0));
    assert_eq!(stepped(Some(0), Older, 3), Some(1));
    assert_eq!(stepped(Some(2), Older, 3), Some(2), "stays at the oldest");
    assert_eq!(stepped(Some(1), Newer, 3), Some(0));
    assert_eq!(stepped(Some(0), Newer, 3), None, "back to what was typed");
    assert_eq!(stepped(None, Newer, 3), None);
    assert_eq!(stepped(None, Older, 0), None, "nothing to step through");
}

fn answer(at: usize, words: &[&str]) -> Completions {
    Completions {
        at,
        words: words.iter().map(|w| (*w).to_owned()).collect(),
    }
}

#[test]
fn one_word_is_put_in_and_several_are_offered_with_what_they_share() {
    // (what, the line, the answer, the line after, the words offered)
    let cases: [(&str, &str, Completions, &str, &[&str]); 6] = [
        ("nothing to complete", "zzz", answer(0, &[]), "zzz", &[]),
        (
            "one command: a space for what comes next",
            "ech",
            answer(0, &["echo"]),
            "echo ",
            &[],
        ),
        (
            "one folder: left open for what is in it",
            "cd sr",
            answer(3, &["src/"]),
            "cd src/",
            &[],
        ),
        (
            "several: what they share is put in",
            "git ch",
            answer(4, &["checkout", "cherry-pick"]),
            "git che",
            &["checkout", "cherry-pick"],
        ),
        (
            "several sharing no more than was typed: the line stays",
            "git c",
            answer(4, &["checkout", "commit"]),
            "git c",
            &["checkout", "commit"],
        ),
        (
            "a place past the line: the line stays",
            "ls",
            answer(9, &["x"]),
            "ls",
            &[],
        ),
    ];
    for (what, line, completions, after, offered) in cases {
        let (text, choices) = completed(line, completions);
        assert_eq!(text, after, "{what}");
        let words: Vec<&str> = choices
            .as_ref()
            .map(|c| c.words().iter().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(words, offered, "{what}");
    }
}

#[test]
fn a_word_picked_from_several_takes_the_place_of_the_lines_end() {
    let (text, choices) = completed("git ch", answer(4, &["checkout", "cherry-pick"]));
    let choices = choices.expect("offered");
    assert_eq!(
        with_word(&choices.completions, &text, &choices.words()[1]).as_deref(),
        Some("git cherry-pick ")
    );
}
