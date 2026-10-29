//! Real shells mark their commands: one block per command, with its output and exit status.

use temor_core::block::Outcome;

use super::super::support::tools_present;
use super::support::{blocks, marks_of, pump, run_line, shell_path, shells, start, start_with};
use temor_ui::input::Stroke;
use temor_vt::{KeyCode, KeyInput, Modifiers};

#[test]
fn real_shells_make_one_block_per_command_with_its_output_and_status() {
    if !tools_present() {
        return;
    }
    let _alone = super::super::support::heavy();
    for (name, shell) in shells() {
        let mut live = start(&format!("blocks-{name}"), &shell);
        run_line(&live, "echo hello-block");
        pump(&mut live, "the first block", |s| {
            s.block_outcomes().len() == 1 && s.block_outcomes()[0].1 != Outcome::Running
        });
        run_line(&live, "sh -c \"exit 1\"");
        pump(&mut live, "the second block", |s| {
            s.block_outcomes().len() == 2 && s.block_outcomes()[1].1 != Outcome::Running
        });
        let found = blocks(&live);
        assert_eq!(
            found,
            vec![
                ("echo hello-block".to_owned(), Outcome::Succeeded),
                (
                    "sh -c \"exit 1\"".to_owned(),
                    Outcome::Failed(temor_core::vocab::ids::ExitCode(1))
                ),
            ],
            "{name}"
        );
        let first = live.session.block_outcomes()[0].0;
        assert_eq!(
            live.session.block_output(first).as_deref(),
            Some("hello-block"),
            "{name}: the output between the marks"
        );
    }
}

#[test]
fn zsh_right_prompt_is_not_part_of_the_command() {
    if !tools_present() {
        return;
    }
    let Some(zsh) = shell_path("zsh") else {
        temor_testkit::skip("zsh", "the zsh right prompt");
        return;
    };
    let mut live = start_with("blocks-rprompt", &zsh, |home| {
        std::fs::write(home.join(".zshrc"), "RPROMPT='RIGHT-PROMPT'\n").expect("zshrc");
    });
    live.session.pane().send(b"echo rp-test");
    // Enter comes once the typing is on screen, as it does for a person.
    pump(&mut live, "the typed line", |s| {
        s.screen_text().contains("echo rp-test")
    });
    // Enter is the person's key: it goes through the session, which notes where the line ends.
    let enter = KeyInput::press(KeyCode::Enter, Modifiers::NONE, "");
    live.session
        .key(Stroke {
            input: None,
            key: enter,
        })
        .expect("enter");
    pump(&mut live, "the block", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    let id = live.session.block_outcomes()[0].0;
    assert_eq!(live.session.block_command(id), Some("echo rp-test"));
    assert_eq!(live.session.block_output(id).as_deref(), Some("rp-test"));
}

#[test]
fn a_program_that_clears_the_scrollback_takes_only_its_own_rows() {
    if !tools_present() {
        return;
    }
    let Some(bash) = shell_path("bash") else {
        temor_testkit::skip("bash", "clearing the screen");
        return;
    };
    let mut live = start("blocks-clear", &bash);
    run_line(&live, "echo before");
    pump(&mut live, "the first block", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    // A frame taller than the screen, then the screen and the scrollback cleared, as Claude Code
    // does when it redraws: only the frame goes, the block before it and its own line stay.
    let clearing = "seq 80; printf '\\033[2J\\033[3J\\033[H'";
    run_line(&live, clearing);
    pump(&mut live, "the prompt after it", |s| s.prompts_seen() >= 3);
    assert_eq!(
        blocks(&live)
            .into_iter()
            .map(|(command, _)| command)
            .collect::<Vec<_>>(),
        ["echo before", clearing]
    );
    let first = live.session.block_outcomes()[0].0;
    assert_eq!(live.session.block_output(first).as_deref(), Some("before"));
}

#[test]
fn the_persons_own_clear_takes_every_block() {
    if !tools_present() {
        return;
    }
    let Some(bash) = shell_path("bash") else {
        temor_testkit::skip("bash", "clearing the screen");
        return;
    };
    let mut live = start("blocks-person-clear", &bash);
    run_line(&live, "echo before");
    pump(&mut live, "the first block and the prompt after it", |s| {
        s.prompts_seen() >= 2
            && s.block_outcomes()
                .first()
                .is_some_and(|b| b.1 != Outcome::Running)
    });
    run_line(&live, "clear");
    pump(&mut live, "the prompt after it", |s| s.prompts_seen() >= 3);
    let left = blocks(&live);
    assert!(
        left.iter().all(|(command, _)| command != "echo before"),
        "a block the person cleared away is still there: {left:?}"
    );
}

#[test]
fn the_shells_screen_clear_takes_the_blocks_it_blanked_and_the_next_command_is_whole() {
    if !tools_present() {
        return;
    }
    let Some(bash) = shell_path("bash") else {
        temor_testkit::skip("bash", "clearing the screen");
        return;
    };
    let mut live = start("blocks-ctrl-l", &bash);
    run_line(&live, "echo before");
    pump(&mut live, "the first block", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    pump(&mut live, "the next prompt", |s| s.prompts_seen() >= 2);
    // Ctrl+L: readline blanks the screen and draws the prompt again at the top.
    live.session.pane().send(b"\x0c");
    pump(&mut live, "the cleared screen", |s| {
        !s.screen_text().contains("before")
    });
    run_line(&live, "echo after");
    pump(&mut live, "the block after", |s| {
        s.block_outcomes()
            .last()
            .is_some_and(|b| b.1 != Outcome::Running)
            && s.prompts_seen() >= 3
    });
    assert_eq!(
        blocks(&live),
        [("echo after".to_owned(), Outcome::Succeeded)]
    );
    let id = live.session.block_outcomes()[0].0;
    assert_eq!(live.session.block_output(id).as_deref(), Some("after"));
    let marks = marks_of(&live.session);
    assert_eq!(marks.first().map(|m| m.row), Some(0), "{marks:?}");
}

#[test]
fn a_command_of_two_lines_from_the_composer_is_one_block_in_the_list() {
    if !tools_present() {
        return;
    }
    let _alone = super::super::support::heavy();
    for (name, shell) in shells() {
        let mut live = start(&format!("blocks-lines-{name}"), &shell);
        // The prompt, and the shell reading pastes: a line break sent before fish turns
        // bracketed paste on is an Enter, and the first line runs alone.
        pump(&mut live, "the prompt", |s| {
            s.block_list().is_some_and(|l| !l.is_empty())
                && s.pane().with_term(|t| t.paste_mode()) == temor_pane::paste::PasteMode::Bracketed
        });
        live.session
            .set_composer_text("echo first\necho second".to_owned());
        live.session.composer_submit();
        pump(&mut live, "the block", |s| {
            s.block_outcomes()
                .first()
                .is_some_and(|b| b.1 != Outcome::Running)
        });
        let id = live.session.block_outcomes()[0].0;
        assert_eq!(
            live.session.block_command(id),
            Some("echo first\necho second"),
            "{name}: both lines"
        );
        let listed = live.session.block_list().unwrap_or_default();
        assert!(
            listed
                .iter()
                .any(|item| matches!(item, temor_core::session::list::Item::Done(b) if b.id == id)),
            "{name}: the block is in the list: {listed:?}"
        );
    }
}

/// Tab in the composer: bash, zsh and fish complete the line with their own completion, a command
/// name and a folder (left open for what is in it); nushell does not say it answers, so nothing is
/// asked of it and the line stays as typed.
#[test]
fn tab_in_the_composer_completes_the_line_with_the_shells_own_completion() {
    if !tools_present() {
        return;
    }
    let _alone = super::super::support::heavy();
    for (name, shell) in shells() {
        let mut live = start(&format!("blocks-tab-{name}"), &shell);
        pump(&mut live, "the prompt", |s| {
            s.block_list().is_some_and(|l| !l.is_empty())
                && s.pane().with_term(|t| t.paste_mode()) == temor_pane::paste::PasteMode::Bracketed
        });
        live.session.set_composer_shown(true);
        if name == "nu" {
            live.session.set_composer_text("ech".to_owned());
            live.session.composer_complete();
            assert!(!live.session.composer_completes(), "nu: says it answers");
            assert_eq!(live.session.composer_text(), "ech", "nu: the line stays");
            continue;
        }
        pump(&mut live, "the shell saying it answers Tab", |s| {
            s.composer_completes()
        });
        // (the line, what it becomes or starts with)
        for (line, want) in [("ech", "echo"), ("ls /us", "ls /usr/")] {
            live.session.set_composer_text(line.to_owned());
            live.session.composer_complete();
            pump(&mut live, &format!("{name}: {line} completed"), |s| {
                s.composer_text() != line || s.composer_choices().is_some()
            });
            let text = live.session.composer_text().to_owned();
            assert!(text.starts_with(want), "{name}: {line:?} became {text:?}");
            if let Some(choices) = live.session.composer_choices() {
                assert!(
                    choices
                        .words()
                        .iter()
                        .all(|w| w.starts_with(&want[line.rfind(' ').map_or(0, |i| i + 1)..])),
                    "{name}: {:?}",
                    choices.words()
                );
            }
        }
        // Nothing was left in the shell's own line: a command sent now runs as typed.
        live.session.set_composer_text("echo after-tab".to_owned());
        live.session.composer_submit();
        pump(&mut live, &format!("{name}: the command after Tab"), |s| {
            s.block_outcomes()
                .last()
                .is_some_and(|b| b.1 != Outcome::Running)
        });
        let id = live.session.block_outcomes().last().expect("a block").0;
        assert_eq!(
            live.session.block_output(id).as_deref(),
            Some("after-tab"),
            "{name}"
        );
    }
}
