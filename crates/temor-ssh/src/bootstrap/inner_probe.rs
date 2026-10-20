//! Asking a running shell which shell it is, before anything shell-specific is typed (PLAN
//! P17). One short line works in bash, zsh, fish and nushell alike: it starts a plain `sh`,
//! which prints the name of its parent (the shell that ran it) after a fixed word. The screen
//! then holds the typed line, which has `$(` where the name goes, and the answer, which has the
//! name; only the answer counts. The rule is deterministic: nothing is guessed from the typed
//! command.

use temor_shell::PROBE_WORD;

use super::inner_shell::InnerShell;

// The probe's text is temor-shell's: temor.zsh has to match it to keep it out of history.
pub use temor_shell::probe_line;

/// What the screen shows of the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeReading {
    /// No answer yet.
    Silent,
    /// A shell temor has an integration for.
    Known(InnerShell),
    /// Some other shell (dash, ash, ...), or a name that could not be read.
    Other,
}

/// Reads the last answer on `screen`. A line counts when the word is followed by a name made of
/// letters, digits, `.`, `_`, `-` and `/`; the typed probe line (`temor-shell:$(`) never does.
pub fn read_probe(screen: &str) -> ProbeReading {
    match last_answer(screen) {
        None => ProbeReading::Silent,
        Some((_, name)) => {
            InnerShell::from_name(name).map_or(ProbeReading::Other, ProbeReading::Known)
        }
    }
}

/// Whether the shell has drawn something below its last answer: its next prompt. A shell that
/// is still between the answer and that prompt (nushell asks the terminal where its cursor is
/// first) may drop what is typed meanwhile.
pub fn prompt_after_answer(screen: &str) -> bool {
    last_answer(screen).is_some_and(|(at, _)| {
        screen
            .lines()
            .skip(at + 1)
            .any(|line| !line.trim().is_empty())
    })
}

/// The last answer on `screen`: its line number and the name it gives.
fn last_answer(screen: &str) -> Option<(usize, &str)> {
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/');
    screen
        .lines()
        .enumerate()
        .filter_map(|(at, line)| {
            let (_, rest) = line.rsplit_once(PROBE_WORD)?;
            let name = rest.trim();
            (!name.is_empty() && name.chars().all(plain)).then_some((at, name))
        })
        .last()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_names_the_shell_and_the_typed_line_does_not() {
        let typed = probe_line();
        let typed = typed.trim_end();
        let rows: [(&str, String, ProbeReading); 9] = [
            ("nothing", "$ ".to_owned(), ProbeReading::Silent),
            (
                "only the typed line",
                typed.to_owned(),
                ProbeReading::Silent,
            ),
            (
                "bash",
                format!("{typed}\ntemor-shell:bash\n$ "),
                ProbeReading::Known(InnerShell::Bash),
            ),
            (
                "a login zsh",
                format!("{typed}\ntemor-shell:-zsh"),
                ProbeReading::Known(InnerShell::Zsh),
            ),
            (
                "fish with a path",
                "temor-shell:/usr/bin/fish".to_owned(),
                ProbeReading::Known(InnerShell::Fish),
            ),
            (
                "nushell",
                format!("{typed}\ntemor-shell:nu\n>"),
                ProbeReading::Known(InnerShell::Nu),
            ),
            (
                "dash",
                format!("{typed}\ntemor-shell:dash"),
                ProbeReading::Other,
            ),
            (
                "the newest answer wins",
                "temor-shell:dash\n$ again\ntemor-shell:zsh".to_owned(),
                ProbeReading::Known(InnerShell::Zsh),
            ),
            (
                "an answer with junk is no answer",
                "temor-shell:ba sh!".to_owned(),
                ProbeReading::Silent,
            ),
        ];
        for (name, screen, want) in rows {
            assert_eq!(read_probe(&screen), want, "{name}");
        }
    }

    #[test]
    fn the_next_prompt_counts_only_below_the_last_answer() {
        let rows = [
            ("no answer", "$ ", false),
            ("the answer is the last line", "x\ntemor-shell:nu", false),
            ("blank lines below it", "temor-shell:nu\n\n   \n", false),
            ("a prompt below it", "temor-shell:bash\n$ ", true),
            (
                "a prompt below an older answer only",
                "temor-shell:sh\n$ \ntemor-shell:nu\n",
                false,
            ),
        ];
        for (name, screen, want) in rows {
            assert_eq!(prompt_after_answer(screen), want, "{name}");
        }
    }

    #[test]
    fn the_probe_line_is_short_and_safe_to_type_into_any_shell() {
        let line = probe_line();
        assert!(line.starts_with(" sh -c '") && line.ends_with("' sh\n"));
        let inside = &line[" sh -c '".len()..line.len() - "' sh\n".len()];
        for bad in ['\'', '\\', '!'] {
            assert!(!inside.contains(bad), "{bad:?}");
        }
        assert!(line.len() < 200);
    }
}
