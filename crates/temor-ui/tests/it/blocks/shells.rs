//! Real shells mark their commands: one block per command, with its output and exit status.

use temor_core::block::Outcome;

use super::super::support::tools_present;
use super::support::{blocks, pump, run_line, shell_path, shells, start, start_with};
use temor_ui::input::Stroke;
use temor_vt::{KeyCode, KeyInput, Modifiers};

#[test]
fn real_shells_make_one_block_per_command_with_its_output_and_status() {
    if !tools_present() {
        return;
    }
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
        assert!(
            !std::env::var("TEMOR_REQUIRE_TOOLS").is_ok_and(|v| v == "1"),
            "zsh is required for the right-prompt test (TEMOR_REQUIRE_TOOLS=1)"
        );
        println!("SKIPPED: zsh right prompt needs zsh (set TEMOR_REQUIRE_TOOLS=1 to fail instead)");
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
            chord: None,
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
