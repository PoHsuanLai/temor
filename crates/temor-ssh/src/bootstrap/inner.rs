//! The line that loads temor's shell integration into a shell that is already running (PLAN
//! P17): typed into a pane after the person said Yes, when an inner `ssh`, `docker exec` or
//! `sudo -i` shell shows no blocks. Unlike the bootstrap it starts no new shell and writes no
//! file: the scripts travel inside the line, base64-encoded.

use temor_shell::ShellScript;

use super::base64;
use crate::error::SshError;

/// The longest line built (the scripts are about 14 KiB once encoded).
pub const MAX_INNER_LINE_BYTES: usize = 64 * 1024;

/// One line, ending in a newline, that bash and zsh run to load the integration. fish and any
/// other shell run it and nothing happens (fish's script ends with `exit`, which would close the
/// session if it were run in place).
///
/// The line starts with a space: bash with `HISTCONTROL=ignorespace` or `ignoreboth` and zsh
/// with `HIST_IGNORE_SPACE` keep it out of history. Other shells, and bash without that setting,
/// record it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnerLoadLine(String);

impl InnerLoadLine {
    /// Builds the line from `integration` (`temor_shell::integration_scripts()`); only
    /// `temor.bash` and `temor.zsh` are used.
    pub fn build(integration: &[ShellScript]) -> Result<Self, SshError> {
        let payload = |name: &str| {
            integration
                .iter()
                .find(|script| script.name == name)
                .map(|script| base64::encode(wrapped(script.contents).as_bytes()))
                .unwrap_or_default()
        };
        let zsh = payload("temor.zsh");
        let bash = payload("temor.bash");
        // The shell is told apart by the version variables the shells set themselves. `sh` picks
        // the payload, so one line parses the same in bash, zsh and fish. Nothing inside the
        // single quotes is a quote or a backslash.
        let decode = "{ base64 -d 2>/dev/null || openssl base64 -d -A 2>/dev/null; }";
        let pick = format!(
            "case \"$1\" in ?*) printf %s {zsh} | {decode};; *) case \"$2\" in ?*) printf %s {bash} | {decode};; esac;; esac"
        );
        let line = format!(" eval \"$(sh -c '{pick}' sh \"$ZSH_VERSION\" \"$BASH_VERSION\")\"\n");
        match line.len() <= MAX_INNER_LINE_BYTES {
            true => Ok(Self(line)),
            false => Err(SshError::ScriptTooLarge {
                bytes: line.len(),
                limit: MAX_INNER_LINE_BYTES,
            }),
        }
    }

    /// The line to type, newline included.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The script inside a function, so its early `return` (already loaded, not interactive) ends
/// the load instead of failing at the top level of `eval`.
fn wrapped(script: &str) -> String {
    format!(
        "_temor_offer_load() {{\n{}\n}}\n_temor_offer_load\nunset -f _temor_offer_load\n",
        script.trim_end_matches('\n')
    )
}
