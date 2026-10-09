//! Assembles the bootstrap from the embedded template and the integration scripts, and wraps
//! it as the remote command.

use temor_pty::ShellScript;

use super::files::{TERMINFO_SOURCE, remote_path};
use super::quote::{heredoc_write, sh_single_quote};
use crate::argv::RemoteCommand;
use crate::error::SshError;

/// The template, with `#@INTEGRATION@` and `#@TERMINFO@` lines replaced by heredocs.
const TEMPLATE: &str = include_str!("../../sh/bootstrap.sh");
const INTEGRATION_MARK: &str = "#@INTEGRATION@";
const TERMINFO_MARK: &str = "#@TERMINFO@";

/// Linux refuses a single exec argument over 128 KiB, and the script travels as one `sh -c`
/// argument, so the bootstrap must stay well under that.
const MAX_SCRIPT_BYTES: usize = 96 * 1024;

/// The complete POSIX `sh` script, ready to be quoted into a remote command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapScript(String);

impl BootstrapScript {
    /// Builds the script with `integration` (from `temor_pty::integration_scripts()`) written
    /// to the temp directory. Scripts this bootstrap does not place are left out.
    pub fn build(integration: &[ShellScript]) -> Result<Self, SshError> {
        let mut placed = String::new();
        for script in integration {
            if let Some(rel) = remote_path(script) {
                placed.push_str(&heredoc_write(
                    &format!("\"$temor_tmp/{rel}\""),
                    script.contents,
                )?);
            }
        }
        let terminfo = heredoc_write("\"$temor_tmp/xterm-ghostty.src\"", TERMINFO_SOURCE)?;
        let mut script = String::with_capacity(TEMPLATE.len() + placed.len() + terminfo.len());
        for line in TEMPLATE.lines() {
            match line.trim() {
                INTEGRATION_MARK => script.push_str(&placed),
                TERMINFO_MARK => script.push_str(&terminfo),
                _ => {
                    script.push_str(line);
                    script.push('\n');
                }
            }
        }
        match script.len() <= MAX_SCRIPT_BYTES {
            true => Ok(Self(script)),
            false => Err(SshError::ScriptTooLarge {
                bytes: script.len(),
                limit: MAX_SCRIPT_BYTES,
            }),
        }
    }

    /// The script text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `sh -c '<script>'`, with the script quoted so nothing in it reaches the login shell's own
/// parsing. The login shell only has to understand single quotes.
pub fn bootstrap_remote_command(script: &BootstrapScript) -> RemoteCommand {
    RemoteCommand::new(format!("sh -c {}", sh_single_quote(script.as_str())))
}
