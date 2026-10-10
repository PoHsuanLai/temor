//! What each inner shell is given: its own integration script, changed to run in place, and
//! the code that takes this offer's lines out of its history afterwards. The scripts are the
//! ones temor ships; the changes are small and made on the text here, so one copy of each script
//! stays the truth.

use temor_shell::ShellScript;

use super::inner::LoadToken;

/// A shell the load text can reach. Any other shell gets nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InnerShell {
    Bash,
    Zsh,
    Fish,
    Nu,
}

impl InnerShell {
    pub(super) const ALL: [InnerShell; 4] = [Self::Bash, Self::Zsh, Self::Fish, Self::Nu];

    /// The `case` pattern for the shell's process name.
    pub(super) fn pattern(self) -> &'static str {
        match self {
            Self::Bash => "bash*",
            Self::Zsh => "zsh*",
            Self::Fish => "fish*",
            Self::Nu => "nu*",
        }
    }

    fn script_name(self) -> &'static str {
        match self {
            Self::Bash => "temor.bash",
            Self::Zsh => "temor.zsh",
            Self::Fish => "temor.fish",
            Self::Nu => "temor.nu",
        }
    }

    /// The file the shell sources, or `None` when `integration` lacks its script.
    pub(super) fn payload(self, integration: &[ShellScript], token: &LoadToken) -> Option<String> {
        let script = integration
            .iter()
            .find(|script| script.name == self.script_name())?;
        let path = token.path();
        Some(match self {
            Self::Bash => format!(
                "{}{}",
                in_function(&slim(script.contents)),
                bash_forget(token, &path)
            ),
            Self::Zsh => format!(
                "{}{}",
                in_function(&slim(script.contents)),
                zsh_forget(&path)
            ),
            Self::Fish => format!(
                "{}{}",
                fish_in_place(script.contents),
                fish_forget(token, &path)
            ),
            Self::Nu => format!("{}^rm -f {path}\n", nu_in_place(script.contents)),
        })
    }
}

/// `text` without comment lines and blank lines, which only cost typing.
pub(super) fn slim(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .flat_map(|line| [line, "\n"])
        .collect()
}

/// The script inside a function, so its early `return` (already loaded, not interactive) ends
/// the load instead of failing at the top level of `source`.
fn in_function(body: &str) -> String {
    format!(
        "_temor_offer_load() {{\n{}\n}}\n_temor_offer_load\nunset -f _temor_offer_load\n",
        body.trim_end_matches('\n')
    )
}

/// fish: the part that takes temor's entry out of `XDG_DATA_DIRS` belongs to the autoload
/// start and is left out; `exit` becomes `return` inside a function, so it ends the load and
/// not the person's shell.
fn fish_in_place(script: &str) -> String {
    let kept = cut_between(
        script,
        "# Take temor's entry back out",
        "status is-interactive; or exit 0",
    );
    let body = slim(&kept).replace("exit 0", "return 0");
    format!(
        "function _temor_offer_load\n{}\nend\n_temor_offer_load\nfunctions -e _temor_offer_load\n",
        body.trim_end_matches('\n')
    )
}

/// nushell: only the lines that switch its own marks on; the rest restores a variable that
/// temor's start sets, which is not set here.
fn nu_in_place(script: &str) -> String {
    let head = script
        .split("# Take temor's folder back out")
        .next()
        .unwrap_or_default();
    slim(head)
}

/// `text` without the span from the line holding `from` up to, not including, the line
/// holding `to`. Unchanged when either is missing.
fn cut_between(text: &str, from: &str, to: &str) -> String {
    match (text.find(from), text.find(to)) {
        (Some(start), Some(end)) if start < end => format!("{}{}", &text[..start], &text[end..]),
        _ => text.to_owned(),
    }
}

/// bash: delete this offer's entries by number (the first lines of `history` output), then
/// remove the file. Entries the shell never recorded are not in the output, so nothing else goes.
fn bash_forget(token: &LoadToken, path: &str) -> String {
    format!(
        "_temor_offer_forget() {{
    local HISTTIMEFORMAT= num rest i
    local -a nums=()
    while read -r num rest; do
        [[ $num =~ ^[0-9]+$ ]] || continue
        [[ $rest == *{marker}* || $rest == *_temor_hs_off=1* ]] && nums+=(\"$num\")
    done < <(history 64)
    for ((i = ${{#nums[@]}} - 1; i >= 0; i--)); do history -d \"${{nums[i]}}\"; done
}}
_temor_offer_forget
unset -f _temor_offer_forget
command rm -f -- {path}
",
        marker = token.marker()
    )
}

/// zsh: lines that start with a space are already leaving history (the first line made it so);
/// put the option back as it was, then remove the file.
fn zsh_forget(path: &str) -> String {
    format!(
        "if [[ -n ${{_temor_hs_off-}} ]]; then
    unsetopt histignorespace
    unset _temor_hs_off
fi
command rm -f -- {path}
"
    )
}

/// fish: after this command finishes, find this offer's entries (the search ends each with a
/// NUL, so a command of many lines stays one entry) and delete them exactly.
fn fish_forget(token: &LoadToken, path: &str) -> String {
    format!(
        "function _temor_offer_forget --on-event fish_postexec
    functions -e _temor_offer_forget
    set -l hits
    builtin history search --null --max 8 | while read --null --local entry
        if string match -q -- '*{marker}*' $entry; or string match -q -- '*_temor_hs_off=1*' $entry
            set -a hits $entry
        end
    end
    for entry in $hits
        builtin history delete --exact --case-sensitive -- $entry
    end
end
command rm -f -- {path}
",
        marker = token.marker()
    )
}
