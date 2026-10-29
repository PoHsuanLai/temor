//! Putting the helper on a host and finding out what is there (PLAN §9b, Tier B). The commands
//! are built and read here as plain text; `RemoteWork` runs them over the shared connection.
//! The helper lives at `~/.cache/temor-remote/<hash>/temor-remote`, one folder per build, so an
//! update never overwrites a build that sessions still run.

use std::path::{Path, PathBuf};

use temor_core::vocab::text::HelperHash;
use temor_hosts::remote::{CacheDir, HostArch, HostOs, Probe};
use temor_ssh::{ControlLayout, HostAlias};
use temor_wire::hello::{HELPER_CACHE_FOLDER, HelperVersion};

use crate::remote::builds::arch_of;
use temor_wire::digest::ContentHash;

/// Where the helper build `hash` lives on the host (the login shell expands the `~`).
pub fn helper_path(hash: &ContentHash) -> String {
    format!("~/.cache/{}", HelperVersion::cache_path(*hash))
}

/// The helper build this temor carries: its bytes and their hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperBlob {
    pub hash: ContentHash,
    pub bytes: std::sync::Arc<Vec<u8>>,
}

impl HelperBlob {
    /// The build's identity as the pure steps know it.
    pub fn name(&self) -> HelperHash {
        HelperHash(self.hash.to_hex())
    }
}

/// Reads the helper build from `path`. `None` when there is none there (a temor without the
/// helper beside it keeps every host on Tier A).
pub fn read_blob(path: &Path) -> Option<HelperBlob> {
    let bytes = std::fs::read(path).ok().filter(|bytes| !bytes.is_empty())?;
    let hash = temor_remote::files::sha256(&bytes);
    Some(HelperBlob {
        hash,
        bytes: std::sync::Arc::new(bytes),
    })
}

/// Where the helper build for this computer's own kind of processor is looked for:
/// `TEMOR_REMOTE_HELPER`, else `temor-remote` beside the running program. The builds for other
/// kinds are looked for beside it (see `remote_builds`).
pub fn blob_path(explicit: Option<&std::ffi::OsStr>, exe: Option<&Path>) -> Option<PathBuf> {
    match explicit.filter(|path| !path.is_empty()) {
        Some(path) => Some(PathBuf::from(path)),
        None => exe
            .and_then(Path::parent)
            .map(|dir| dir.join("temor-remote")),
    }
}

/// The `ssh` arguments for a one-off command on `alias`: no terminal, no password prompt, on the
/// shared connection.
pub fn command_argv(alias: &HostAlias, layout: &ControlLayout, remote: &str) -> Vec<String> {
    vec![
        "-T".to_owned(),
        "-o".to_owned(),
        "BatchMode=yes".to_owned(),
        "-o".to_owned(),
        layout.control_path_option(),
        "--".to_owned(),
        alias.as_str().to_owned(),
        remote.to_owned(),
    ]
}

/// The script that reports the host's system, processor, whether the helper's folder is
/// writable, the login variables a session needs, and the builds already there.
pub const PROBE_SCRIPT: &str = "sh -c 'uname -s; uname -m; d=\"$HOME/.cache/temor-remote\"; \
mkdir -p \"$d\" 2>/dev/null; if [ -w \"$d\" ]; then echo writable; else echo readonly; fi; \
env | grep -E \"^(HOME|USER|LOGNAME|SHELL|PATH|LANG|LC_ALL)=\" | sed \"s/^/env:/\"; \
for f in \"$d\"/*/temor-remote; do [ -x \"$f\" ] && basename \"$(dirname \"$f\")\"; done; true'";

/// The host's own login variables: the helper adds nothing to a session's environment, so the
/// window passes these on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostLogin(pub Vec<(String, String)>);

impl HostLogin {
    /// The host's home folder, when it said.
    pub fn home(&self) -> Option<&str> {
        self.0
            .iter()
            .find(|(name, _)| name == "HOME")
            .map(|(_, value)| value.as_str())
            .filter(|home| home.starts_with('/'))
    }

    /// The whole environment of a session: the host's variables, and what a terminal says
    /// about itself.
    pub fn session_env(&self) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = self
            .0
            .iter()
            .filter(|(name, _)| name != "TERM" && name != "COLORTERM")
            .cloned()
            .collect();
        env.push(("TERM".to_owned(), "xterm-256color".to_owned()));
        env.push(("COLORTERM".to_owned(), "truecolor".to_owned()));
        env
    }
}

/// What probing a host found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub probe: Probe,
    pub login: HostLogin,
}

/// Reads the probe's output. `ours` are the builds this temor carries (`HelperBuilds::names`):
/// of the builds installed on the host, one of ours is the one reported, else any.
pub fn parse_probe(text: &str, ours: &[HelperHash]) -> Found {
    let mut lines = text.lines().map(str::trim);
    let os = match lines.next() {
        Some("Linux") => HostOs::Linux,
        _ => HostOs::Other,
    };
    let arch = lines.next().map_or(HostArch::Other, arch_of);
    let cache = match lines.next() {
        Some("writable") => CacheDir::Usable,
        _ => CacheDir::Unusable,
    };
    let mut login = Vec::new();
    let mut found: Vec<HelperHash> = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.strip_prefix("env:").and_then(|v| v.split_once('=')) {
            login.push((name.to_owned(), value.to_owned()));
        } else if ContentHash::from_hex(line).is_some() {
            found.push(HelperHash(line.to_ascii_lowercase()));
        }
    }
    let installed = found
        .iter()
        .find(|hash| ours.contains(hash))
        .or_else(|| found.first())
        .cloned();
    Found {
        probe: Probe {
            os,
            arch,
            cache,
            installed,
        },
        login: HostLogin(login),
    }
}

/// The command that stores what it reads as the build `hash` and prints the hash of what it
/// stored. The file appears under its name only once it is whole and executable.
pub fn install_script(hash: &ContentHash) -> String {
    let dir = format!("$HOME/.cache/{HELPER_CACHE_FOLDER}/{}", hash.to_hex());
    format!(
        "sh -c 'set -e; d=\"{dir}\"; mkdir -p \"$d\"; cat > \"$d/temor-remote.part\"; \
chmod 755 \"$d/temor-remote.part\"; mv \"$d/temor-remote.part\" \"$d/temor-remote\"; \
sha256sum \"$d/temor-remote\" | cut -d\" \" -f1'"
    )
}

/// The command that removes the build `hash` from the host.
pub fn remove_script(hash: &ContentHash) -> String {
    format!(
        "sh -c 'rm -rf \"$HOME/.cache/{HELPER_CACHE_FOLDER}/{}\"'",
        hash.to_hex()
    )
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
