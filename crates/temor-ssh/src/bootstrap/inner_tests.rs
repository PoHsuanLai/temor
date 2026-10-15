//! The text that loads the integration into a running shell: its lines, what may stand between
//! its quotes, and what each shell is given. Running it in real shells is in `tests/it`.

use temor_shell::{ShellScript, integration_scripts};

use super::inner::{PASTE, ZSH_PRELUDE, chunk_lines};
use super::inner_shell::InnerShell;
use super::{InnerLoadLine, LoadToken, MAX_INNER_LINE_BYTES, MAX_TTY_LINE_BYTES};
use crate::error::SshError;

const MARKER: &str = "temor-load-0123456789abcdeffedcba9876543210";

fn token() -> LoadToken {
    LoadToken::from_words(0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210)
}

/// The text for every shell the `sh` program reaches (nushell gets its own, below).
fn built() -> Vec<(InnerShell, InnerLoadLine)> {
    InnerShell::ALL
        .into_iter()
        .filter(|&shell| shell != InnerShell::Nu)
        .map(|shell| {
            let text = InnerLoadLine::build_for(shell, &integration_scripts(), &token());
            (shell, text.expect("build"))
        })
        .collect()
}

#[test]
fn text_is_cut_into_runs_that_fit() {
    let cases: [(&str, usize, &[usize]); 6] = [
        ("empty text has no run", 5, &[]),
        ("shorter than a run", 5, &[3]),
        ("exactly one run", 5, &[5]),
        ("one over", 5, &[5, 1]),
        ("several runs and a rest", 5, &[5, 5, 5, 2]),
        ("a zero width still makes progress", 0, &[1, 1, 1]),
    ];
    for (name, width, runs) in cases {
        let text = "a".repeat(runs.iter().sum());
        let got: Vec<usize> = chunk_lines(&text, width).iter().map(String::len).collect();
        assert_eq!(got, runs, "{name}");
    }
}

#[test]
fn every_typed_line_fits_a_terminal_in_line_mode() {
    for (shell, line) in built() {
        let text = line.as_str();
        assert!(text.len() <= MAX_INNER_LINE_BYTES, "{shell:?}");
        assert!(text.is_ascii(), "{shell:?}");
        for typed in line.lines() {
            // The newline counts toward the 4096 bytes a line-mode terminal keeps.
            assert!(
                typed.len() < MAX_TTY_LINE_BYTES,
                "{shell:?}: {} bytes",
                typed.len()
            );
        }
        assert!(
            line.lines().count() > 5,
            "{shell:?}: the payload is split across lines"
        );
    }
}

#[test]
fn the_program_between_the_quotes_holds_no_quote_backslash_or_history_mark() {
    for (shell, line) in built() {
        let text = line.as_str();
        let start = text.find("sh -c '").expect("program") + "sh -c '".len();
        let end = text.rfind("' sh\n").expect("end of program");
        let inside = &text[start..end];
        for bad in ['\'', '\\', '!'] {
            assert!(
                !inside.contains(bad),
                "{shell:?}: {bad:?} between the quotes"
            );
        }
    }
}

#[test]
fn the_order_is_prelude_then_program_then_source() {
    assert!(
        ZSH_PRELUDE.starts_with(' '),
        "a space keeps it out of history"
    );
    for (shell, line) in built() {
        let text = line.as_str();
        assert_eq!(text.starts_with(ZSH_PRELUDE), shell == InnerShell::Zsh);
        assert!(text.ends_with(&format!("\n source /tmp/{MARKER}\n")));
        assert!(text.contains(&format!("\nf=/tmp/{MARKER}\n")));
        let own = format!("{}) dec >$f <<E\n", shell.pattern());
        for arm in [own.as_str(), "*) : >$f ;;\n"] {
            assert!(text.contains(arm), "{shell:?}: no arm {arm:?}");
        }
    }
}

#[test]
fn each_shell_gets_its_own_script_and_its_own_way_to_forget() {
    let cases: [(InnerShell, &[&str], &[&str]); 4] = [
        (
            InnerShell::Bash,
            &[
                "_temor_offer_load() {",
                "history -d",
                "*temor-shell:*",
                MARKER,
                "command rm -f -- /tmp/temor-load-",
            ],
            &["XDG_DATA_DIRS"],
        ),
        (
            InnerShell::Zsh,
            &[
                "_temor_offer_load() {",
                "unsetopt histignorespace",
                "add-zsh-hook precmd",
            ],
            &["history -d"],
        ),
        (
            InnerShell::Fish,
            &[
                "function _temor_offer_load",
                "return 0",
                "history delete --exact",
                "*temor-shell:*",
                MARKER,
            ],
            &["exit", "XDG_DATA_DIRS", "TEMOR_FISH"],
        ),
        (
            InnerShell::Nu,
            &["osc133 = true", "osc7 = true"],
            &["XDG_DATA_DIRS", "history"],
        ),
    ];
    for (shell, has, hasnt) in cases {
        let payload = shell
            .payload(&integration_scripts(), &token())
            .expect("script present");
        for want in has {
            assert!(payload.contains(want), "{shell:?} lacks {want:?}");
        }
        for unwanted in hasnt {
            assert!(!payload.contains(unwanted), "{shell:?} holds {unwanted:?}");
        }
        assert!(!payload.contains("\n#"), "{shell:?}: comments were left in");
    }
}

#[test]
fn a_missing_script_leaves_its_shell_out_and_too_much_text_is_refused() {
    let scripts: Vec<ShellScript> = integration_scripts()
        .into_iter()
        .filter(|script| script.name != "temor.nu")
        .collect();
    let text = InnerLoadLine::build_for(InnerShell::Nu, &scripts, &token()).expect("build");
    assert!(!text.as_str().contains("nu*) "));

    let big: &'static str = Box::leak("echo x\n".repeat(20_000).into_boxed_str());
    let scripts = [ShellScript {
        name: "temor.bash",
        contents: big,
    }];
    let err =
        InnerLoadLine::build_for(InnerShell::Bash, &scripts, &token()).expect_err("too large");
    assert!(matches!(err, SshError::ScriptTooLarge { .. }), "{err}");
}

#[test]
fn text_for_a_named_shell_holds_only_what_that_shell_reads() {
    // (shell, starts with the zsh first line, the arms it holds, the arms it lacks)
    let cases: [(InnerShell, bool, &str, [&str; 3]); 3] = [
        (
            InnerShell::Bash,
            false,
            "bash*) ",
            ["zsh*) ", "fish*) ", "nu*) "],
        ),
        (
            InnerShell::Zsh,
            true,
            "zsh*) ",
            ["bash*) ", "fish*) ", "nu*) "],
        ),
        (
            InnerShell::Fish,
            false,
            "fish*) ",
            ["bash*) ", "zsh*) ", "nu*) "],
        ),
    ];
    for (shell, prelude, has, hasnt) in cases {
        let text = InnerLoadLine::build_for(shell, &integration_scripts(), &token())
            .expect("build")
            .as_str()
            .to_owned();
        assert_eq!(text.starts_with(ZSH_PRELUDE), prelude, "{shell:?}");
        assert!(text.contains(has), "{shell:?} lacks {has:?}");
        for arm in hasnt {
            assert!(!text.contains(arm), "{shell:?} holds {arm:?}");
        }
    }
}

/// nushell is given its script itself as one paste and one Enter: no `sh` program, no file.
#[test]
fn nushell_gets_its_script_as_one_pasted_command() {
    let text =
        InnerLoadLine::build_for(InnerShell::Nu, &integration_scripts(), &token()).expect("build");
    let text = text.as_str();
    let inside = text
        .strip_prefix(PASTE.0)
        .and_then(|rest| rest.strip_suffix(&format!("{}\n", PASTE.1)))
        .expect("one paste, then Enter");
    assert!(!inside.contains(PASTE.0) && !inside.contains(PASTE.1));
    let payload = InnerShell::Nu
        .payload(&integration_scripts(), &token())
        .expect("script present");
    assert_eq!(inside, payload.trim_end_matches('\n'));
    for unwanted in ["sh -c", "<<E", "/tmp/", "source"] {
        assert!(!text.contains(unwanted), "holds {unwanted:?}");
    }
}

#[test]
fn process_names_map_to_shells() {
    let rows = [
        ("bash", Some(InnerShell::Bash)),
        ("-zsh", Some(InnerShell::Zsh)),
        ("/usr/bin/fish", Some(InnerShell::Fish)),
        ("nu", Some(InnerShell::Nu)),
        ("dash", None),
        ("", None),
    ];
    for (name, want) in rows {
        assert_eq!(InnerShell::from_name(name), want, "{name:?}");
    }
}

#[test]
fn a_command_line_names_its_shell_only_by_its_exact_last_word() {
    // (command line, listed command) -> the shell it names
    let rows = [
        (
            "docker exec -it web zsh",
            "docker exec",
            Some(InnerShell::Zsh),
        ),
        (
            "kubectl exec -it pod -- bash",
            "kubectl exec",
            Some(InnerShell::Bash),
        ),
        ("ssh -t host /usr/bin/fish", "ssh", Some(InnerShell::Fish)),
        (
            "TERM=xterm docker run -it ubuntu nu",
            "docker run -it",
            Some(InnerShell::Nu),
        ),
        ("ssh bash", "ssh", None),
        ("ssh host", "ssh", None),
        ("docker exec -it web zsh -l", "docker exec", None),
        ("docker exec -it web zsh-5.9", "docker exec", None),
        ("docker exec -it web sh", "docker exec", None),
        ("sudo -s", "sudo -s", None),
    ];
    for (command, program, want) in rows {
        assert_eq!(InnerShell::named_in(command, program), want, "{command:?}");
    }
}
