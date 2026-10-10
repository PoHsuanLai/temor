//! The text that loads the integration into a running shell: its lines, what may stand between
//! its quotes, and what each shell is given. Running it in real shells is in `tests/it`.

use temor_shell::{ShellScript, integration_scripts};

use super::inner::{ZSH_PRELUDE, chunk_lines};
use super::inner_shell::InnerShell;
use super::{InnerLoadLine, LoadToken, MAX_INNER_LINE_BYTES, MAX_TTY_LINE_BYTES};
use crate::error::SshError;

const MARKER: &str = "temor-load-0123456789abcdeffedcba9876543210";

fn token() -> LoadToken {
    LoadToken::from_words(0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210)
}

fn built() -> InnerLoadLine {
    InnerLoadLine::build(&integration_scripts(), &token()).expect("build")
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
    let line = built();
    let text = line.as_str();
    assert!(text.len() <= MAX_INNER_LINE_BYTES);
    assert!(text.is_ascii());
    for typed in line.lines() {
        // The newline counts toward the 4096 bytes a line-mode terminal keeps.
        assert!(typed.len() < MAX_TTY_LINE_BYTES, "{} bytes", typed.len());
    }
    assert!(
        line.lines().count() > 5,
        "the payload is split across lines"
    );
}

#[test]
fn the_program_between_the_quotes_holds_no_quote_backslash_or_history_mark() {
    let text = built().as_str().to_owned();
    let start = text.find("sh -c '").expect("program") + "sh -c '".len();
    let end = text.rfind("' sh\n").expect("end of program");
    let inside = &text[start..end];
    for bad in ['\'', '\\', '!'] {
        assert!(!inside.contains(bad), "{bad:?} between the quotes");
    }
}

#[test]
fn the_order_is_prelude_then_program_then_source() {
    let text = built().as_str().to_owned();
    assert!(text.starts_with(ZSH_PRELUDE));
    assert!(
        ZSH_PRELUDE.starts_with(' '),
        "a space keeps it out of history"
    );
    assert!(text.ends_with(&format!("\n source /tmp/{MARKER}\n")));
    assert!(text.contains(&format!("\nf=/tmp/{MARKER}\n")));
    for arm in [
        "bash*) dec >$f <<E\n",
        "zsh*) ",
        "fish*) ",
        "nu*) ",
        "*) : >$f ;;\n",
    ] {
        assert!(text.contains(arm), "no arm {arm:?}");
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
                MARKER,
            ],
            &["exit", "XDG_DATA_DIRS", "TEMOR_FISH"],
        ),
        (
            InnerShell::Nu,
            &["osc133 = true", "osc7 = true", "^rm -f /tmp/temor-load-"],
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
    let text = InnerLoadLine::build(&scripts, &token()).expect("build");
    assert!(!text.as_str().contains("nu*) "));

    let big: &'static str = Box::leak("echo x\n".repeat(20_000).into_boxed_str());
    let scripts = [ShellScript {
        name: "temor.bash",
        contents: big,
    }];
    let err = InnerLoadLine::build(&scripts, &token()).expect_err("too large");
    assert!(matches!(err, SshError::ScriptTooLarge { .. }), "{err}");
}
