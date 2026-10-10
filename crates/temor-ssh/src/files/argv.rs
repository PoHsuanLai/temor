//! The `ssh` and `scp` argv for file operations on the shared connection. They join the
//! master through its control socket and never start a connection of their own: with no master
//! the command fails (`ControlMaster=no`, `BatchMode=yes`) instead of asking for a password.

use std::path::{Path, PathBuf};

use super::path::RemoteFilePath;
use crate::argv::ControlLayout;
use crate::bootstrap::wrap_script;
use crate::config::HostAlias;
use crate::error::SshError;

/// The programs to run, injected so tests can point them at stand-ins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTools {
    pub ssh: PathBuf,
    pub scp: PathBuf,
}

impl Default for FileTools {
    /// `ssh` and `scp` found through the caller's PATH.
    fn default() -> Self {
        Self {
            ssh: PathBuf::from("ssh"),
            scp: PathBuf::from("scp"),
        }
    }
}

fn through_master(layout: &ControlLayout) -> Vec<String> {
    ["-o", "BatchMode=yes", "-o", "ControlMaster=no", "-o"]
        .into_iter()
        .map(str::to_owned)
        .chain([layout.control_path_option()])
        .collect()
}

/// Single-quotes `text` for `sh`. The result only ever sits inside a script that travels
/// base64-encoded, so the host's login shell never reads it.
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// `ssh` arguments that print `missing`, or `<mtime> <size>` for `path` (GNU stat, else BSD).
pub fn stat_argv(
    alias: &HostAlias,
    layout: &ControlLayout,
    path: &RemoteFilePath,
) -> Result<Vec<String>, SshError> {
    let script = format!(
        "p={}; if [ ! -e \"$p\" ]; then echo missing; else stat -c '%Y %s' -- \"$p\" 2>/dev/null || stat -f '%m %z' -- \"$p\"; fi",
        sh_quote(path.as_str())
    );
    let remote = wrap_script(&script)?;
    let mut argv = vec!["-T".to_owned()];
    argv.extend(through_master(layout));
    argv.extend([
        "--".to_owned(),
        alias.as_str().to_owned(),
        remote.as_str().to_owned(),
    ]);
    Ok(argv)
}

/// `scp` arguments that copy `path` on the host to the absolute `local` path.
pub fn fetch_argv(
    alias: &HostAlias,
    layout: &ControlLayout,
    path: &RemoteFilePath,
    local: &Path,
) -> Result<Vec<String>, SshError> {
    let local = local_text(local)?;
    let mut argv = scp_base(layout);
    argv.extend([format!("{}:{}", alias.as_str(), path.as_str()), local]);
    Ok(argv)
}

/// `scp` arguments that copy the absolute `local` file over `path` on the host, in place, so
/// the host keeps the file's owner and mode.
pub fn store_argv(
    alias: &HostAlias,
    layout: &ControlLayout,
    local: &Path,
    path: &RemoteFilePath,
) -> Result<Vec<String>, SshError> {
    let local = local_text(local)?;
    let mut argv = scp_base(layout);
    argv.extend([local, format!("{}:{}", alias.as_str(), path.as_str())]);
    Ok(argv)
}

/// `-s` makes scp use the file-transfer protocol, so no remote shell reads the path.
fn scp_base(layout: &ControlLayout) -> Vec<String> {
    let mut argv = vec!["-q".to_owned(), "-s".to_owned()];
    argv.extend(through_master(layout));
    argv.push("--".to_owned());
    argv
}

/// A local path as scp reads it: absolute (so a `:` can never make it look like a host), UTF-8,
/// no control characters.
fn local_text(path: &Path) -> Result<String, SshError> {
    match path.to_str() {
        Some(text) if path.is_absolute() && !text.chars().any(char::is_control) => {
            Ok(text.to_owned())
        }
        _ => Err(SshError::BadLocalPath(path.display().to_string())),
    }
}
