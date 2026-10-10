//! Asking a running shell which shell it is, before anything shell-specific is typed (PLAN
//! P17). One short line works in bash, zsh, fish and nushell alike: it starts a plain `sh`,
//! which prints the name of its parent (the shell that ran it) after a fixed word. The screen
//! then holds the typed line, which has `$(` where the name goes, and the answer, which has the
//! name; only the answer counts. The rule is deterministic: nothing is guessed from the typed
//! command.

use super::inner_shell::InnerShell;

/// The word that leads the answer.
pub(super) const PROBE_WORD: &str = "temor-shell:";

/// The line to type, newline included. Leading space: shells that ignore such lines keep it
/// out of history. Nothing between the quotes is a quote, a backslash or a history mark.
pub fn probe_line() -> String {
    format!(
        " sh -c 'echo {PROBE_WORD}$(cat /proc/$PPID/comm 2>/dev/null || ps -p $PPID -o comm= 2>/dev/null)' sh\n"
    )
}

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
    let answer = screen.lines().rev().find_map(|line| {
        let (_, rest) = line.rsplit_once(PROBE_WORD)?;
        let name = rest.trim();
        let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/');
        (!name.is_empty() && name.chars().all(plain)).then_some(name)
    });
    match answer {
        None => ProbeReading::Silent,
        Some(name) => InnerShell::from_name(name).map_or(ProbeReading::Other, ProbeReading::Known),
    }
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
