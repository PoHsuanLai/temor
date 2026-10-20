//! The default prompt: temor's in place of the shell's, whatever the person's startup files set,
//! unless TEMOR_PROMPT=shell keeps theirs. The folder and the git branch are on a header line and
//! the end mark at column 0 of the line below it.

use std::path::{Path, PathBuf};

use temor_shell::{ChildEnv, Shell, ShellStart};

use crate::support::{Scratch, Term, find, program_or_skip, setsid, size};

const PROMPT_START: &[u8] = b"\x1b]133;A";
// The marks end with ST or BEL as the shell chooses (fish 4 changed which), so only the
// mark itself is matched.
const PROMPT_END: &[u8] = b"\x1b]133;B";

const SHELLS: [&str; 4] = ["bash", "zsh", "fish", "nu"];

/// What the person's own startup file sets: nothing, or a prompt of their own.
#[derive(Clone, Copy, PartialEq)]
enum Own {
    Stock,
    Custom,
}

/// The person's startup file for `shell` (path under HOME, contents).
fn startup(shell: &str, own: Own) -> (&'static str, &'static str) {
    match (shell, own) {
        ("bash", Own::Stock) => (".bashrc", ""),
        ("bash", Own::Custom) => (".bashrc", "PS1='custom> '\n"),
        // An empty .zshrc keeps zsh's first-run wizard away.
        ("zsh", Own::Stock) => (".zshrc", ""),
        ("zsh", Own::Custom) => (".zshrc", "PS1='custom> '\nRPS1='right<'\n"),
        ("nu", Own::Stock) => (".config/nushell/config.nu", ""),
        ("nu", Own::Custom) => (
            ".config/nushell/config.nu",
            "$env.PROMPT_COMMAND = {|| \"custom> \" }\n$env.PROMPT_COMMAND_RIGHT = {|| \"right<\" }\n",
        ),
        (_, Own::Stock) => (".config/fish/config.fish", ""),
        (_, Own::Custom) => (
            ".config/fish/config.fish",
            "function fish_prompt\n    echo -n 'custom> '\nend\nfunction fish_right_prompt\n    echo -n 'right<'\nend\n",
        ),
    }
}

/// Starts `shell` in `cwd` and returns it once the first prompt's end mark has arrived.
fn start(shell: &str, scratch: &Scratch, own: Own, profile: &ChildEnv, cwd: PathBuf) -> Term {
    let (file, contents) = startup(shell, own);
    let path = scratch.home.join(file);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("startup dir");
    std::fs::write(path, contents).expect("startup file");
    let program = program_or_skip(shell, "prompt").expect("checked by the caller");
    let spec = temor_pty::prepare_shell(
        &Shell::at(program),
        ShellStart::Interactive,
        scratch.contract_env(profile),
        &scratch.runtime,
        cwd,
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(
        term.read_until_has(0, "\x1b]133;B"),
        "{shell}: no first prompt: {:?}",
        term.text()
    );
    term
}

/// `bytes` without terminal control sequences or carriage returns.
fn plain(bytes: &[u8]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            0x1b if bytes.get(i + 1) == Some(&b'[') => {
                i += 2;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
            }
            0x1b if bytes.get(i + 1) == Some(&b']') => {
                i += 2;
                while i < bytes.len() && bytes[i] != 0x07 && bytes[i] != 0x1b {
                    i += 1;
                }
                if bytes.get(i) == Some(&0x1b) {
                    i += 1;
                }
            }
            // A character set chosen (`ESC ( B`, which fish 3 sends with every colour reset).
            0x1b if matches!(bytes.get(i + 1), Some(b'(' | b')')) => i += 2,
            0x1b => i += 1,
            b'\r' => {}
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The last prompt drawn so far, from its A mark to its B mark, without control sequences.
fn last_prompt(term: &Term) -> String {
    let out = &term.out;
    let end = out
        .windows(PROMPT_END.len())
        .rposition(|w| w == PROMPT_END)
        .expect("a B mark");
    let start = out[..end]
        .windows(PROMPT_START.len())
        .rposition(|w| w == PROMPT_START)
        .expect("an A mark before it");
    plain(&out[start..end])
}

/// Whether the prompt is temor's: the end mark starts the line below a header.
fn is_temor_prompt(prompt: &str) -> bool {
    prompt.contains('\n') && prompt.ends_with('\n')
}

/// The header line: the last line before the one the end mark is on.
fn header(prompt: &str) -> &str {
    prompt
        .trim_end_matches('\n')
        .rsplit('\n')
        .next()
        .unwrap_or("")
}

fn folder_name(cwd: &Path) -> String {
    cwd.file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned()
}

/// A command that fails with status 1 in `shell` (nushell's own `false` is a value, not a failure).
fn failing(shell: &str) -> &'static str {
    match shell {
        "nu" => "^false",
        _ => "false",
    }
}

/// Runs `command` and waits until its D mark (with `status`) and a new prompt's B mark arrive.
fn run_command(term: &mut Term, command: &str, status: u8) {
    let from = term.out.len();
    term.send(&format!("{command}\r"));
    // The status, then the mark's end: ST or BEL.
    let ends = [
        format!("\x1b]133;D;{status}\x1b\\"),
        format!("\x1b]133;D;{status}\x07"),
    ];
    let found = |out: &[u8]| {
        ends.iter()
            .find_map(|done| find(&out[from..], done.as_bytes()))
    };
    assert!(
        term.read_until(|out| found(out).is_some()),
        "no D: {:?}",
        term.text()
    );
    let at = found(&term.out).expect("D") + from;
    assert!(
        term.read_until(|out| find(&out[at..], PROMPT_END).is_some()),
        "no prompt after {command}: {:?}",
        term.text()
    );
}

/// Each case: (name, own prompt, TEMOR_PROMPT value, expected to be replaced).
#[test]
fn temors_prompt_replaces_any_and_shell_keeps_the_persons() {
    if crate::support::pty_unsupported("temors_prompt_replaces_any_and_shell_keeps_the_persons") {
        return;
    }
    let cases = [
        ("stock prompt", Own::Stock, None, true),
        ("custom prompt", Own::Custom, None, true),
        (
            "custom, TEMOR_PROMPT=shell",
            Own::Custom,
            Some("shell"),
            false,
        ),
        (
            "stock, TEMOR_PROMPT=shell",
            Own::Stock,
            Some("shell"),
            false,
        ),
        ("TEMOR_PROMPT=temor", Own::Custom, Some("temor"), true),
    ];
    for shell in SHELLS {
        if program_or_skip(
            shell,
            "temors_prompt_replaces_any_and_shell_keeps_the_persons",
        )
        .is_none()
        {
            continue;
        }
        for (name, own, value, replaced) in cases {
            let scratch = Scratch::new();
            let profile: ChildEnv = value.map(|v| ("TEMOR_PROMPT", v)).into_iter().collect();
            let term = start(shell, &scratch, own, &profile, scratch.cwd.clone());
            let prompt = last_prompt(&term);
            assert_eq!(
                is_temor_prompt(&prompt),
                replaced,
                "{shell}, {name}: {prompt:?}"
            );
            if own == Own::Custom {
                // The person's prompt shows exactly when temor's does not; so does a right-side one.
                assert_eq!(
                    prompt.contains("custom> "),
                    !replaced,
                    "{shell}, {name}: {prompt:?}"
                );
                if replaced {
                    let screen = term.text();
                    assert!(!screen.contains("right<"), "{shell}, {name}: {screen:?}");
                }
            }
        }
    }
}

#[test]
fn the_header_names_the_folder_and_the_branch() {
    if crate::support::pty_unsupported("the_header_names_the_folder_and_the_branch") {
        return;
    }
    // (name, .git as a file or a directory, HEAD contents, expected branch)
    let cases = [
        ("a branch", false, "ref: refs/heads/feat/x\n", "feat/x"),
        (
            "a detached head",
            false,
            "0123456789abcdef0123\n",
            "0123456",
        ),
        (
            "a worktree",
            true,
            "ref: refs/heads/wt-branch\n",
            "wt-branch",
        ),
    ];
    for shell in SHELLS {
        if program_or_skip(shell, "the_header_names_the_folder_and_the_branch").is_none() {
            continue;
        }
        for (name, is_file, head, branch) in cases {
            let scratch = Scratch::new();
            let git = scratch.cwd.join(".git");
            if is_file {
                let real = scratch.cwd.join("real-git/worktrees/wt");
                std::fs::create_dir_all(&real).expect("worktree dir");
                std::fs::write(real.join("HEAD"), head).expect("HEAD");
                let line = format!("gitdir: {}\n", real.display());
                std::fs::write(&git, line).expect(".git file");
            } else {
                std::fs::create_dir_all(&git).expect(".git");
                std::fs::write(git.join("HEAD"), head).expect("HEAD");
            }
            let term = start(
                shell,
                &scratch,
                Own::Stock,
                &ChildEnv::new(),
                scratch.cwd.clone(),
            );
            let prompt = last_prompt(&term);
            let line = header(&prompt);
            assert!(
                line.contains(&folder_name(&scratch.cwd)) && line.ends_with(&format!("  {branch}")),
                "{shell}, {name}: header {line:?} in {prompt:?}"
            );
        }
    }
}

#[test]
fn home_shows_as_a_tilde() {
    if crate::support::pty_unsupported("home_shows_as_a_tilde") {
        return;
    }
    for shell in SHELLS {
        if program_or_skip(shell, "home_shows_as_a_tilde").is_none() {
            continue;
        }
        let scratch = Scratch::new();
        let term = start(
            shell,
            &scratch,
            Own::Stock,
            &ChildEnv::new(),
            scratch.home.clone(),
        );
        let prompt = last_prompt(&term);
        let line = header(&prompt);
        assert!(
            line == "~" || line.starts_with("~  "),
            "{shell}: header {line:?} in {prompt:?}"
        );
    }
}

/// The first prompt has no blank line above its header, nor has one after an empty line; the
/// ones after a command do. The end
/// mark stays at column 0 of the line below the header after failures and stops alike.
#[test]
fn a_blank_line_separates_prompts_except_the_first() {
    if crate::support::pty_unsupported("a_blank_line_separates_prompts_except_the_first") {
        return;
    }
    for shell in SHELLS {
        if program_or_skip(shell, "a_blank_line_separates_prompts_except_the_first").is_none() {
            continue;
        }
        let scratch = Scratch::new();
        let mut term = start(
            shell,
            &scratch,
            Own::Stock,
            &ChildEnv::new(),
            scratch.cwd.clone(),
        );
        let first = last_prompt(&term);
        assert!(!first.starts_with('\n'), "{shell}: first {first:?}");
        // An empty Enter runs no command, so the prompt after it still has no blank line.
        let from = term.out.len();
        term.send("\r");
        assert!(
            term.read_until(|out| find(&out[from..], PROMPT_END).is_some()),
            "{shell}: no prompt after an empty line: {:?}",
            term.text()
        );
        let after_empty = last_prompt(&term);
        assert!(
            !after_empty.starts_with('\n'),
            "{shell}: after an empty line {after_empty:?}"
        );
        for (command, status) in [(failing(shell), 1), ("sh -c 'exit 130'", 130)] {
            run_command(&mut term, command, status);
            let prompt = last_prompt(&term);
            assert!(
                prompt.starts_with('\n') && is_temor_prompt(&prompt),
                "{shell}, after {command}: {prompt:?}"
            );
            assert!(
                header(&prompt).contains(&folder_name(&scratch.cwd)),
                "{shell}: {prompt:?}"
            );
        }
    }
}
