//! Assembles the bootstrap from the embedded template and the integration scripts, and wraps
//! it as the remote command.

use std::collections::BTreeSet;

use temor_shell::{ShellScript, XDG_DATA_DIRS_DEFAULT, sh_quote};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use super::files::TERMINFO_SOURCE;
use super::quote::heredoc_write;
use crate::argv::RemoteCommand;
use crate::error::SshError;
use crate::files::RemoteFilePath;

/// The template, with `#@INTEGRATION@` and `#@TERMINFO@` lines replaced by heredocs.
const TEMPLATE: &str = include_str!("../../sh/bootstrap.sh");
const DIRS_MARK: &str = "#@DIRS@";
const INTEGRATION_MARK: &str = "#@INTEGRATION@";
const TERMINFO_MARK: &str = "#@TERMINFO@";
const OPTIONS_MARK: &str = "#@OPTIONS@";
/// Where `XDG_DATA_DIRS` points when the host leaves it unset, written inline in the template.
const XDG_DEFAULT_MARK: &str = "#@XDG_DATA_DIRS_DEFAULT@";

/// Whether the bootstrap may add Ghostty's terminal description to the host's `~/.terminfo`
/// when the host lacks it. Without it the host gets `xterm-256color`. This is the one change
/// the bootstrap makes outside its temporary folder, so it is asked for (see temor-core's
/// host consent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TerminfoPolicy {
    /// Install it when `tic` is there and the host lacks it.
    #[default]
    Install,
    /// Never write to the host's home folder.
    Skip,
}

/// What the bootstrap is allowed to do on the host beyond its temporary folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct BootstrapOptions {
    pub terminfo: TerminfoPolicy,
}

impl BootstrapOptions {
    /// Options with the given terminfo policy.
    pub fn with_terminfo(terminfo: TerminfoPolicy) -> Self {
        Self { terminfo }
    }
}

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
        Self::build_with(integration, BootstrapOptions::default())
    }

    /// [`BootstrapScript::build`] with `options` saying what the script may change on the host.
    pub fn build_with(
        integration: &[ShellScript],
        options: BootstrapOptions,
    ) -> Result<Self, SshError> {
        Self::build_in(integration, options, None)
    }

    /// [`BootstrapScript::build_with`] where the shell starts in `start_dir` on the host (a
    /// session opened beside another one, in its folder). A folder that is gone is skipped
    /// and the shell starts where the login does.
    pub fn build_in(
        integration: &[ShellScript],
        options: BootstrapOptions,
        start_dir: Option<&RemoteFilePath>,
    ) -> Result<Self, SshError> {
        let mut placed = String::new();
        let mut folders = BTreeSet::new();
        for script in integration {
            let rel = script.relative_path();
            if let Some((folder, _)) = rel.rsplit_once('/') {
                folders.insert(format!("\"$temor_tmp/{folder}\""));
            }
            placed.push_str(&heredoc_write(
                &format!("\"$temor_tmp/{rel}\""),
                script.contents,
            )?);
        }
        let folders: Vec<String> = folders.into_iter().collect();
        let make_folders = match folders.is_empty() {
            true => String::new(),
            false => format!("mkdir -p {} || exit 1\n", folders.join(" ")),
        };
        let (terminfo, policy) = match options.terminfo {
            TerminfoPolicy::Install => (
                heredoc_write("\"$temor_tmp/xterm-ghostty.src\"", TERMINFO_SOURCE)?,
                "install",
            ),
            TerminfoPolicy::Skip => (String::new(), "skip"),
        };
        let dir = start_dir.map_or_else(String::new, |dir| sh_quote(dir.as_str()));
        let options_line = format!("temor_terminfo={policy}\ntemor_start_dir={dir}\n");
        let mut script = String::with_capacity(TEMPLATE.len() + placed.len() + terminfo.len());
        for line in TEMPLATE.lines() {
            match line.trim() {
                DIRS_MARK => script.push_str(&make_folders),
                INTEGRATION_MARK => script.push_str(&placed),
                TERMINFO_MARK => script.push_str(&terminfo),
                OPTIONS_MARK => script.push_str(&options_line),
                _ => {
                    script.push_str(line);
                    script.push('\n');
                }
            }
        }
        Ok(Self(script.replace(XDG_DEFAULT_MARK, XDG_DATA_DIRS_DEFAULT)))
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
    let encoded = STANDARD.encode(script.as_bytes());
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
