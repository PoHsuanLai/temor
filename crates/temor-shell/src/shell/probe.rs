//! The line temor types into a running shell to ask which shell it is (PLAN P17). It lives here,
//! beside `temor.zsh`, which has to keep it out of history by its text.

/// The word that leads the answer.
pub const PROBE_WORD: &str = "temor-shell:";

/// The line to type, newline included. Leading space: shells that ignore such lines keep it
/// out of history. Nothing between the quotes is a quote, a backslash or a history mark.
pub fn probe_line() -> String {
    format!(
        " sh -c 'echo {PROBE_WORD}$(cat /proc/$PPID/comm 2>/dev/null || ps -p $PPID -o comm= 2>/dev/null)' sh\n"
    )
}

#[cfg(test)]
mod tests {
    use super::{PROBE_WORD, probe_line};
    use crate::shell::scripts::ZSH_SCRIPT;

    #[test]
    fn zsh_keeps_out_of_history_the_line_this_types() {
        // temor.zsh matches the start of the line by its text.
        let start = format!("sh -c 'echo {PROBE_WORD}");
        assert!(probe_line().contains(&start), "{}", probe_line());
        assert!(
            ZSH_SCRIPT.contents.contains(&format!("*\\"{start}\\"*")),
            "temor.zsh no longer matches the probe line"
        );
    }
}
