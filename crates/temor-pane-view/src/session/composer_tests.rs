//! The composer's history from plain lists: no session, no shell.

use temor_panes::shell::complete::Completions;

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
    use super::HistoryStep::{Newer, Older};
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

fn shells(line: &str, at: usize, words: &[&str]) -> Vec<Suggestion> {
    answer(at, words).suggestions(line)
}

#[test]
fn one_word_is_put_in_and_several_are_offered_with_what_they_share() {
    // (what, the line, the suggestions, the line after, the words offered)
    type Case<'a> = (&'a str, &'a str, Vec<Suggestion>, &'a str, &'a [&'a str]);
    let cases: [Case; 6] = [
        (
            "nothing to complete",
            "zzz",
            shells("zzz", 0, &[]),
            "zzz",
            &[],
        ),
        (
            "one command: a space for what comes next",
            "ech",
            shells("ech", 0, &["echo"]),
            "echo ",
            &[],
        ),
        (
            "one folder: left open for what is in it",
            "cd sr",
            shells("cd sr", 3, &["src/"]),
            "cd src/",
            &[],
        ),
        (
            "several: what they share is put in",
            "git ch",
            shells("git ch", 4, &["checkout", "cherry-pick"]),
            "git che",
            &["checkout", "cherry-pick"],
        ),
        (
            "several sharing no more than was typed: the line stays",
            "git c",
            shells("git c", 4, &["checkout", "commit"]),
            "git c",
            &["checkout", "commit"],
        ),
        (
            "a place past the line: the line stays",
            "ls",
            shells("ls", 9, &["x"]),
            "ls",
            &[],
        ),
    ];
    for (what, line, suggestions, after, offered) in cases {
        let (text, choices) = completed(line, suggestions);
        assert_eq!(text, after, "{what}");
        let words: Vec<&str> = choices.as_ref().map(Choices::words).unwrap_or_default();
        assert_eq!(words, offered, "{what}");
    }
}

#[test]
fn suggestions_that_take_different_places_share_nothing() {
    let whole = Suggestion {
        word: "git reset --soft HEAD~1".to_owned(),
        from: 0,
        to: 4,
        space: false,
        detail: Some("undo the last commit".to_owned()),
    };
    let mut suggestions = shells("undo", 0, &["undone"]);
    suggestions.push(whole);
    let (text, choices) = completed("undo", suggestions);
    assert_eq!(text, "undo", "no shared start where the places differ");
    let choices = choices.expect("offered");
    assert_eq!(choices.details(), [None, Some("undo the last commit")]);
}

#[test]
fn a_suggestion_picked_from_several_takes_its_place_in_the_line_asked_about() {
    let (text, choices) = completed("git ch", shells("git ch", 4, &["checkout", "cherry-pick"]));
    assert_eq!(text, "git che", "what they share is in the field");
    let choices = choices.expect("offered");
    let taken = choices.suggestions[1].applied(&choices.base);
    assert_eq!(taken.as_deref(), Some("git cherry-pick "));
}

const SLOW: SourceId = SourceId("slow");

/// A composer with `line` typed and a request to the shell and the slow source out for it.
fn asking(line: &str) -> Composer {
    Composer {
        text: line.to_owned(),
        asked: Some(Asked {
            request: 3,
            line: line.to_owned(),
            waiting: vec![SHELL, SLOW],
            merged: Vec::new(),
            presented: false,
        }),
        ..Composer::default()
    }
}

fn said(source: SourceId, request: u64, suggestions: Vec<Suggestion>) -> Answered {
    Answered {
        request,
        source,
        suggestions,
    }
}

#[test]
fn a_late_source_adds_to_the_list_the_first_one_started() {
    let mut composer = asking("git ch");
    composer.merge(said(
        SHELL,
        3,
        shells("git ch", 4, &["checkout", "cherry-pick"]),
    ));
    assert_eq!(composer.text, "git che");
    assert!(composer.asked.is_some(), "the slow source is still awaited");
    composer.choices.as_mut().expect("offered").picked = 1;

    let whole = Suggestion {
        word: "git checkout main".to_owned(),
        from: 0,
        to: 6,
        space: false,
        detail: Some("switch to main".to_owned()),
    };
    composer.merge(said(
        SLOW,
        3,
        vec![whole, shells("git ch", 4, &["checkout"]).remove(0)],
    ));
    let choices = composer.choices.as_ref().expect("offered");
    assert_eq!(
        choices.words(),
        ["checkout", "cherry-pick", "git checkout main"],
        "what both said once, the first source's first"
    );
    assert_eq!(choices.picked(), 1, "the pick stays where it was");
    assert_eq!(composer.text, "git che", "the line is not changed again");
    assert!(composer.asked.is_none(), "every source has answered");
}

#[test]
fn the_first_answer_with_one_suggestion_completes_and_later_ones_change_nothing() {
    let mut composer = asking("ech");
    composer.merge(said(SLOW, 3, shells("ech", 0, &["echo"])));
    assert_eq!(composer.text, "echo ");
    assert!(
        composer.asked.is_none(),
        "completed: nothing more is taken in"
    );
    composer.merge(said(SHELL, 3, shells("ech", 0, &["echo", "echotc"])));
    assert_eq!(
        (composer.text.as_str(), composer.choices.is_none()),
        ("echo ", true)
    );
}

#[test]
fn an_answer_for_an_old_request_or_a_source_not_asked_is_dropped() {
    let mut composer = asking("ech");
    composer.merge(said(SHELL, 2, shells("ech", 0, &["echo"])));
    composer.merge(said(SourceId("other"), 3, shells("ech", 0, &["echo"])));
    assert_eq!(composer.text, "ech");
    assert!(composer.choices.is_none());
    let mut idle = Composer::default();
    idle.merge(said(SHELL, 1, shells("", 0, &["echo"])));
    assert!(idle.text.is_empty(), "nothing was asked");
}

#[test]
fn sources_that_all_say_nothing_leave_the_line_and_end_the_request() {
    let mut composer = asking("zzz");
    composer.merge(said(SHELL, 3, vec![]));
    assert!(composer.asked.is_some());
    composer.merge(said(SLOW, 3, vec![]));
    assert_eq!(composer.text, "zzz");
    assert!(composer.asked.is_none() && composer.choices.is_none());
}

fn ready_shell(text: &str) -> Composer {
    Composer {
        text: text.to_owned(),
        shell_ready: true,
        ..Composer::default()
    }
}

fn complete(shown: bool, mode: PasteMode, ready: &[SourceId]) -> ComposerIn {
    ComposerIn::Complete {
        shown,
        mode,
        ready: ready.to_vec(),
    }
}

#[test]
fn tab_asks_what_can_answer_and_nothing_while_answers_are_awaited() {
    use PasteMode::{Bracketed, Plain};
    let paste = |text: &str, mode| ComposerOut::Send(paste::encode(text, mode));
    let key = ComposerOut::Send(REQUEST_KEY.to_vec());
    let ask = |line: &str, sources: &[SourceId]| ComposerOut::Ask {
        request: 1,
        line: line.to_owned(),
        sources: sources.to_vec(),
    };
    // (what, the composer, the input, what is asked)
    let cases: Vec<(&str, Composer, ComposerIn, Vec<ComposerOut>)> = vec![
        (
            "the shell: the line as a paste, then the key",
            ready_shell("git ch"),
            complete(true, Plain, &[]),
            vec![paste("git ch", Plain), key.clone()],
        ),
        (
            "an empty line: the key alone",
            ready_shell(""),
            complete(true, Plain, &[]),
            vec![key.clone()],
        ),
        (
            "the shell and a source of its own",
            ready_shell("ls"),
            complete(true, Bracketed, &[SLOW]),
            vec![paste("ls", Bracketed), key.clone(), ask("ls", &[SLOW])],
        ),
        (
            "a line break the shell would run: the source alone",
            ready_shell("a\nb"),
            complete(true, Plain, &[SLOW]),
            vec![ask("a\nb", &[SLOW])],
        ),
        (
            "a shell that has not said it answers, and no source",
            Composer::default(),
            complete(true, Plain, &[]),
            vec![],
        ),
        (
            "the composer not shown",
            ready_shell("ls"),
            complete(false, Plain, &[SLOW]),
            vec![],
        ),
        (
            "answers awaited",
            asking("ls"),
            complete(true, Plain, &[SLOW]),
            vec![],
        ),
    ];
    for (what, mut composer, input, want) in cases {
        let before = composer.asked.clone();
        assert_eq!(composer.step(input), want, "{what}");
        if want.is_empty() {
            assert_eq!(composer.asked, before, "{what}: nothing more awaited");
        }
    }
}

#[test]
fn enter_sends_the_text_and_a_return_and_empties_the_composer() {
    let mut composer = ready_shell("echo hi");
    let out = composer.step(ComposerIn::Submit {
        mode: PasteMode::Plain,
    });
    assert_eq!(
        out,
        [
            ComposerOut::Send(paste::encode("echo hi", PasteMode::Plain)),
            ComposerOut::Send(b"\r".to_vec()),
            ComposerOut::Sent(Some("echo hi".to_owned())),
        ]
    );
    assert_eq!(
        composer,
        Composer::default(),
        "empty, and the shell asked again"
    );
    let out = composer.step(ComposerIn::Submit {
        mode: PasteMode::Plain,
    });
    assert_eq!(
        out,
        [ComposerOut::Send(b"\r".to_vec()), ComposerOut::Sent(None)]
    );
}

#[test]
fn the_arrows_step_through_the_commands_and_typing_starts_again_from_the_text() {
    let commands: Vec<String> = ["ls", "make"].map(str::to_owned).to_vec();
    let mut composer = Composer::default();
    let step = |composer: &mut Composer, step| {
        composer.step(ComposerIn::History {
            step,
            commands: commands.clone(),
        });
        composer.text.clone()
    };
    composer.step(ComposerIn::Append("ec".to_owned()));
    assert_eq!(step(&mut composer, HistoryStep::Older), "make");
    assert_eq!(step(&mut composer, HistoryStep::Older), "ls");
    assert_eq!(step(&mut composer, HistoryStep::Newer), "make");
    assert_eq!(
        step(&mut composer, HistoryStep::Newer),
        "ec",
        "what was typed"
    );
    composer.step(ComposerIn::Append("ho".to_owned()));
    composer.step(ComposerIn::Backspace);
    assert_eq!(composer.text, "ech");
    assert_eq!(step(&mut composer, HistoryStep::Older), "make");
    composer.step(ComposerIn::Edit("pwd".to_owned()));
    assert_eq!(
        step(&mut composer, HistoryStep::Newer),
        "pwd",
        "an edit starts the steps again from it"
    );
}

#[test]
fn a_command_starting_puts_the_words_away_and_the_shell_answers_no_tab() {
    let mut composer = asking("git ch");
    composer.shell_ready = true;
    composer.step(ComposerIn::Shell(Answer::Completed(answer(
        4,
        &["checkout", "cherry-pick"],
    ))));
    assert!(
        composer.choices.is_some(),
        "offered from the shell's answer"
    );
    composer.step(ComposerIn::Pick(HistoryStep::Older));
    assert_eq!(
        composer.choices.as_ref().map(Choices::picked),
        Some(1),
        "round the top"
    );
    composer.step(ComposerIn::Started);
    assert!(!composer.shell_ready && composer.asked.is_none() && composer.choices.is_none());
    composer.step(ComposerIn::Shell(Answer::Ready));
    assert!(composer.shell_ready);
}
