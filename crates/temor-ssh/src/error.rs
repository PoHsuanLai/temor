//! The crate's one error type. Every variant is something a caller can act on.

use std::io;
use std::path::PathBuf;

/// What can go wrong building SSH arguments, scripts and identities, or preparing the
/// runtime directory. Config parsing never fails: its problems are [`crate::ConfigIssue`]s.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SshError {
    /// The text cannot be an `ssh` destination: only ASCII letters, digits and `.-_@:[]%+`,
    /// and no leading `-` (which ssh would read as an option).
    #[error("`{0}` cannot be used as an SSH host name")]
    InvalidAlias(String),
    /// A value cannot go into an `ssh -o` option or a `-L` spec without quoting.
    #[error("`{0}` has characters that cannot be passed to ssh here")]
    UnsafeValue(String),
    /// The shared control socket path is too long for a unix socket (108 bytes including the
    /// terminating NUL, and ssh adds a 17-byte suffix while it sets up a master).
    #[error(
        "the control socket path needs {len} bytes, more than the {limit} a unix socket allows"
    )]
    ControlPathTooLong { len: usize, limit: usize },
    /// A forward's port or host is not usable.
    #[error("invalid port forward: {0}")]
    InvalidForward(String),
    /// A script line would end a heredoc early, so the script cannot be sent as written.
    #[error("the script has a line equal to the heredoc end marker `{0}`")]
    HeredocConflict(String),
    /// Text that should hold an `SHA256:` host key fingerprint does not.
    #[error("`{0}` is not an SHA256 host key fingerprint")]
    BadFingerprint(String),
    /// `ssh-keygen` output held no fingerprint line.
    #[error("no SHA256 host key fingerprint in the ssh-keygen output")]
    NoFingerprint,
    /// The runtime directory exists but another user owns it, or it cannot be made private.
    #[error("the runtime directory {path} is not private to this user: {reason}")]
    RuntimeDirNotPrivate { path: PathBuf, reason: &'static str },
    /// A filesystem operation on the runtime directory failed.
    #[error("cannot prepare {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The remote command is too large to send as one command argument.
    #[error("the remote command is {bytes} bytes, more than the {limit} allowed")]
    ScriptTooLarge { bytes: usize, limit: usize },
    /// A remote file path ssh and scp could not take as written: not absolute, or holding
    /// control or wildcard characters.
    #[error("`{0}` cannot be used as a remote file path")]
    BadRemotePath(String),
    /// A local file path that cannot be handed to scp: not absolute, or holding control characters.
    #[error("`{0}` cannot be used as a local file path here")]
    BadLocalPath(String),
    /// `ssh` or `scp` could not be started.
    #[error("cannot run {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },
    /// A remote file operation ran and failed (no shared connection, no access, no space).
    /// `detail` is the first line the program printed, with control characters removed.
    #[error("{what} failed (exit {code:?}): {detail}")]
    RemoteFile {
        what: &'static str,
        code: Option<i32>,
        detail: String,
    },
    /// The host's answer to a file-details request was not a modified time and a size.
    #[error("the host's file details were not understood: {0}")]
    BadStat(String),
    /// The platform has no private-directory support yet.
    #[error("this platform cannot {0} yet")]
    Unsupported(&'static str),
}
