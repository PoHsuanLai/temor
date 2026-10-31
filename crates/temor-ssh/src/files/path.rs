//! Remote paths and what a host reports about a file.

use crate::error::SshError;
use crate::tailnet::UnixSeconds;

pub use temor_schema::units::ByteCount;

/// An absolute path on the host. No control characters and none of `* ? [ ] { } \`, which
/// scp's file transfer would expand; spaces and quotes are fine because no shell reads them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteFilePath(String);

impl RemoteFilePath {
    /// Validates the path.
    pub fn new(text: &str) -> Result<Self, SshError> {
        let usable = text.starts_with('/')
            && text.len() <= 4096
            && !text
                .chars()
                .any(|c| c.is_control() || matches!(c, '*' | '?' | '[' | ']' | '{' | '}' | '\\'));
        match usable {
            true => Ok(Self(text.to_owned())),
            false => Err(SshError::BadRemotePath(text.to_owned())),
        }
    }

    /// The path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What changes when a file is edited on the host: the modified time (whole seconds) and the
/// size. Two equal stats mean the file is taken to be unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RemoteStat {
    pub mtime: UnixSeconds,
    pub size: ByteCount,
}

/// The host's answer about a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RemoteState {
    Present(RemoteStat),
    Missing,
}

/// Reads the output of the stat script: `missing`, or `<mtime> <size>`, on the last non-empty
/// line (anything printed before it, such as a login banner, is ignored).
pub fn parse_stat(output: &str) -> Result<RemoteState, SshError> {
    let line = output
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or_default();
    if line == "missing" {
        return Ok(RemoteState::Missing);
    }
    let bad = || SshError::BadStat(line.chars().filter(|c| !c.is_control()).take(80).collect());
    let mut parts = line.split(' ');
    let (Some(mtime), Some(size), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(bad());
    };
    match (mtime.parse::<u64>(), size.parse::<u64>()) {
        (Ok(mtime), Ok(size)) => Ok(RemoteState::Present(RemoteStat {
            mtime: UnixSeconds(mtime),
            size: ByteCount(size),
        })),
        _ => Err(bad()),
    }
}
