//! Where the program word of a command line is, when wrappers run it: `env`, `bwrap`, `nice`,
//! `nohup`, `setsid`, `stdbuf` and `timeout`. Each wrapper's options are read by its own grammar,
//! so an option's argument is never taken for the program (`env -u claude zsh` runs `zsh`). An
//! option a grammar does not know ends the search: no program word is better than a wrong one.

/// The file name of a command word: what follows the last `/`.
pub(super) fn file_name(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// The index of the program `tokens` runs, looking through wrappers. `None` when a wrapper's
/// options cannot be read or it has no program.
pub(super) fn program_at(tokens: &[String]) -> Option<usize> {
    let mut at = 0;
    loop {
        let word = tokens.get(at)?;
        let next = match file_name(word) {
            "env" => env(tokens, at + 1),
            "bwrap" => bwrap(tokens, at + 1),
            "nice" => nice(tokens, at + 1),
            "nohup" => plain(tokens, at + 1),
            "setsid" => setsid(tokens, at + 1),
            "stdbuf" => stdbuf(tokens, at + 1),
            "timeout" => timeout(tokens, at + 1),
            _ => return Some(at),
        };
        at = next?;
    }
}

/// `at` when there is a word there.
fn word_at(tokens: &[String], at: usize) -> Option<usize> {
    (at < tokens.len()).then_some(at)
}

pub(super) fn is_option(word: &str) -> bool {
    word.starts_with('-') && word.len() > 1
}

/// A wrapper with no options of its own.
fn plain(tokens: &[String], mut at: usize) -> Option<usize> {
    if tokens.get(at).is_some_and(|w| w == "--") {
        at += 1;
    }
    let word = tokens.get(at)?;
    (!is_option(word)).then_some(at)
}

fn env(tokens: &[String], mut at: usize) -> Option<usize> {
    loop {
        let word = tokens.get(at)?.as_str();
        at += match word {
            // Assignments still come after `--`: `env -- A=b zsh` runs `zsh`.
            "--" => return past_assignments(tokens, at + 1),
            "-" | "-i" | "--ignore-environment" | "-0" | "--null" => 1,
            "-u" | "--unset" | "-C" | "--chdir" => 2,
            _ if word.starts_with("--unset=") || word.starts_with("--chdir=") => 1,
            // `-S` splits a string into words: its program cannot be told. Any other option is
            // unknown.
            _ if is_option(word) => return None,
            _ if word.contains('=') => 1,
            _ => return Some(at),
        };
    }
}

/// The first word from `at` on that is not a `NAME=value` assignment.
fn past_assignments(tokens: &[String], at: usize) -> Option<usize> {
    let skipped = tokens[at.min(tokens.len())..]
        .iter()
        .take_while(|w| w.contains('='))
        .count();
    word_at(tokens, at + skipped)
}

/// How many arguments a `bwrap` option takes.
fn bwrap_arity(option: &str) -> Option<usize> {
    const NONE: &[&str] = &[
        "--help",
        "--version",
        "--unshare-all",
        "--unshare-user",
        "--unshare-user-try",
        "--unshare-ipc",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-uts",
        "--unshare-cgroup",
        "--unshare-cgroup-try",
        "--share-net",
        "--die-with-parent",
        "--new-session",
        "--as-pid-1",
        "--clearenv",
        "--disable-userns",
        "--assert-userns-disabled",
    ];
    const ONE: &[&str] = &[
        "--chdir",
        "--hostname",
        "--uid",
        "--gid",
        "--unsetenv",
        "--dev",
        "--proc",
        "--tmpfs",
        "--mqueue",
        "--dir",
        "--remount-ro",
        "--exec-label",
        "--file-label",
        "--seccomp",
        "--info-fd",
        "--json-status-fd",
        "--block-fd",
        "--userns-block-fd",
        "--sync-fd",
        "--lock-file",
        "--cap-add",
        "--cap-drop",
        "--perms",
        "--size",
        "--userns",
        "--userns2",
        "--pidns",
        "--args",
        "--add-seccomp-fd",
        "--argv0",
        "--overlay-src",
        "--tmp-overlay",
        "--ro-overlay",
    ];
    const TWO: &[&str] = &[
        "--setenv",
        "--bind",
        "--bind-try",
        "--dev-bind",
        "--dev-bind-try",
        "--ro-bind",
        "--ro-bind-try",
        "--symlink",
        "--file",
        "--bind-data",
        "--ro-bind-data",
        "--chmod",
        "--bind-fd",
        "--ro-bind-fd",
    ];
    const THREE: &[&str] = &["--overlay"];
    [(NONE, 0), (ONE, 1), (TWO, 2), (THREE, 3)]
        .into_iter()
        .find_map(|(names, arity)| names.contains(&option).then_some(arity))
}

fn bwrap(tokens: &[String], mut at: usize) -> Option<usize> {
    loop {
        let word = tokens.get(at)?.as_str();
        match word {
            "--" => return word_at(tokens, at + 1),
            _ if word.starts_with('-') => at += 1 + bwrap_arity(word)?,
            _ => return Some(at),
        }
    }
}

fn nice(tokens: &[String], mut at: usize) -> Option<usize> {
    loop {
        let word = tokens.get(at)?.as_str();
        at += match word {
            "--" => return word_at(tokens, at + 1),
            "-n" | "--adjustment" => 2,
            _ if word.starts_with("--adjustment=") => 1,
            // `-n5` and `-5`.
            _ if word.starts_with("-n") && word.len() > 2 => 1,
            _ if is_option(word) && word[1..].bytes().all(|b| b.is_ascii_digit()) => 1,
            _ if is_option(word) => return None,
            _ => return Some(at),
        };
    }
}

fn setsid(tokens: &[String], mut at: usize) -> Option<usize> {
    loop {
        let word = tokens.get(at)?.as_str();
        at += match word {
            "--" => return word_at(tokens, at + 1),
            "-c" | "--ctty" | "-f" | "--fork" | "-w" | "--wait" => 1,
            _ if is_option(word) => return None,
            _ => return Some(at),
        };
    }
}

fn stdbuf(tokens: &[String], mut at: usize) -> Option<usize> {
    loop {
        let word = tokens.get(at)?.as_str();
        at += match word {
            "--" => return word_at(tokens, at + 1),
            "-i" | "-o" | "-e" | "--input" | "--output" | "--error" => 2,
            _ if ["--input=", "--output=", "--error="]
                .iter()
                .any(|p| word.starts_with(p)) =>
            {
                1
            }
            // `-iL`, `-o0`.
            _ if ["-i", "-o", "-e"].iter().any(|p| word.starts_with(p)) => 1,
            _ if is_option(word) => return None,
            _ => return Some(at),
        };
    }
}

fn timeout(tokens: &[String], mut at: usize) -> Option<usize> {
    loop {
        let word = tokens.get(at)?.as_str();
        at += match word {
            "--" => return program_after_duration(tokens, at + 1),
            "-s" | "--signal" | "-k" | "--kill-after" => 2,
            "--foreground" | "--preserve-status" | "-v" | "--verbose" => 1,
            _ if word.starts_with("--signal=") || word.starts_with("--kill-after=") => 1,
            _ if word.starts_with("-s") || word.starts_with("-k") => 1,
            _ if is_option(word) => return None,
            _ => break,
        };
    }
    program_after_duration(tokens, at)
}

/// `at` is the duration; the program follows it.
fn program_after_duration(tokens: &[String], at: usize) -> Option<usize> {
    tokens.get(at)?;
    word_at(tokens, at + 1)
}
