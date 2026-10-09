//! Assembles the bootstrap from the embedded template and the integration scripts, and wraps
//! it as the remote command.

use temor_shell::ShellScript;

use super::base64;
use super::files::{TERMINFO_SOURCE, remote_path};
use super::quote::heredoc_write;
use crate::argv::RemoteCommand;
use crate::error::SshError;

/// The template, with `#@INTEGRATION@` and `#@TERMINFO@` lines replaced by heredocs.
const TEMPLATE: &str = include_str!("../../sh/bootstrap.sh");
const INTEGRATION_MARK: &str = "#@INTEGRATION@";
const TERMINFO_MARK: &str = "#@TERMINFO@";

/// Linux refuses one exec argument over 128 KiB (MAX_ARG_STRLEN, NUL included). The remote
/// command reaches the login shell as one argument, so the whole command must stay under that.
pub const MAX_REMOTE_COMMAND_BYTES: usize = 120 * 1024;

/// The complete POSIX `sh` script, before it is encoded for the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapScript(String);

impl BootstrapScript {
    /// Builds the script with `integration` (from `temor_shell::integration_scripts()`) written
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
        Ok(Self(script))
    }

    /// The script text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The remote command for `script`. The login shell only sees plain ASCII: the script travels
/// base64-encoded and `sh` decodes it, so nu, fish, zsh and POSIX shells all pass it through
/// unchanged. Fails when the final command would not fit one exec argument.
pub fn bootstrap_remote_command(script: &BootstrapScript) -> Result<RemoteCommand, SshError> {
    wrap_script(script.as_str())
}

/// The transport on its own: `script` as a remote command that decodes and runs it with `sh`.
/// The bootstrap uses it, and tests use it to check the transport with a known payload.
#[doc(hidden)]
pub fn wrap_script(script: &str) -> Result<RemoteCommand, SshError> {
    let encoded = base64::encode(script.as_bytes());
    // Only base64 characters and plain shell syntax sit between the quotes (no `'` or `\`). The
    // decoder falls back to openssl for hosts without GNU or BusyBox base64, and a host with
    // neither says so and fails instead of ending the session silently.
    let text = format!(
        "sh -c 's=$(printf %s {encoded} | {{ base64 -d 2>/dev/null || openssl base64 -d -A 2>/dev/null; }}); [ -n \"$s\" ] || {{ echo \"temor: this computer needs base64 or openssl to start\" >&2; exit 1; }}; eval \"$s\"'"
    );
    match text.len() <= MAX_REMOTE_COMMAND_BYTES {
        true => Ok(RemoteCommand::new(text)),
        false => Err(SshError::ScriptTooLarge {
            bytes: text.len(),
            limit: MAX_REMOTE_COMMAND_BYTES,
        }),
    }
}
