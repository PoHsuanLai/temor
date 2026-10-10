//! The config vocabulary: validated newtypes, the host rows and the lenient problems list.

use std::path::{Path, PathBuf};

use crate::error::SshError;

/// Whether `text` is safe to hand to ssh as a destination or to show as a host name: non-empty,
/// at most 255 bytes, ASCII letters, digits and `.-_@:[]%+` only, and no leading `-` (which ssh
/// would read as an option). Nothing a shell would interpret gets through.
fn plain_word(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('-')
        && text.len() <= 255
        && text.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '.' | '-' | '_' | '@' | ':' | '[' | ']' | '%' | '+')
        })
}

/// A concrete `Host` name from the config, or a destination typed in ⌘T. Only a bare name or
/// `user@name`: ssh reads `:port` and `[addr]` as part of the host, so a port must be given
/// separately (`-p`), not inside the destination.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HostAlias(String);

impl HostAlias {
    /// Validates `text` as a destination.
    pub fn new(text: &str) -> Result<Self, SshError> {
        let bare = !text.contains([':', '[', ']']);
        match plain_word(text) && bare {
            true => Ok(Self(text.to_owned())),
            false => Err(SshError::InvalidAlias(text.to_owned())),
        }
    }

    /// The text as ssh receives it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A `HostName` value: where the alias really connects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostName(String);

impl HostName {
    /// Validates a `HostName` value.
    pub fn new(text: &str) -> Option<Self> {
        plain_word(text).then(|| Self(text.to_owned()))
    }

    /// The fallback when a config has no `HostName`: the alias is the name.
    pub(super) fn from_alias(alias: &HostAlias) -> Self {
        Self(alias.as_str().to_owned())
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A `User` value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserName(String);

impl UserName {
    /// Validates a `User` value.
    pub fn new(text: &str) -> Option<Self> {
        plain_word(text).then(|| Self(text.to_owned()))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A TCP port from 1 to 65535.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Port(u16);

impl Port {
    /// Parses a port; `None` for zero, anything out of range, or non-digits.
    pub fn parse(text: &str) -> Option<Self> {
        match text.parse::<u16>() {
            Ok(n) if n > 0 => Some(Self(n)),
            _ => None,
        }
    }

    /// The number.
    pub fn get(self) -> u16 {
        self.0
    }
}

/// An `IdentityFile` value, kept as written (`~` is expanded by [`IdentityFile::expanded`]).
/// Only length and control characters are checked here. A builder that passes it to `ssh -i`
/// must go through [`IdentityFile::for_argv`] (a value starting with `-` is an option to ssh).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityFile(String);

impl IdentityFile {
    /// Validates an `IdentityFile` value.
    pub fn new(text: &str) -> Option<Self> {
        let usable = !text.is_empty() && !text.chars().any(char::is_control);
        usable.then(|| Self(text.to_owned()))
    }

    /// The path as written in the config.
    pub fn as_written(&self) -> &str {
        &self.0
    }

    /// The path with a leading `~/` replaced by the injected `home`. `None` when `~` is used
    /// and there is no home.
    pub fn expanded(&self, home: Option<&Path>) -> Option<PathBuf> {
        match self.0.strip_prefix("~/") {
            Some(rest) => home.map(|h| h.join(rest)),
            None if self.0 == "~" => home.map(Path::to_path_buf),
            None => Some(PathBuf::from(&self.0)),
        }
    }
}

/// A `ProxyJump` value (one or more hops, comma-separated), kept as written. Only control
/// characters are checked here; a builder that passes it to `ssh -J` must go through
/// [`ProxyJump::for_argv`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProxyJump(String);

impl ProxyJump {
    /// Validates a `ProxyJump` value.
    pub fn new(text: &str) -> Option<Self> {
        let usable = !text.is_empty() && !text.chars().any(char::is_control);
        usable.then(|| Self(text.to_owned()))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A config line number, counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LineNo(pub u32);

/// One concrete host from the config, with its effective settings. Settings follow OpenSSH:
/// the first value obtained wins, and `IdentityFile` lines accumulate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    pub alias: HostAlias,
    pub hostname: HostName,
    pub user: Option<UserName>,
    pub port: Option<Port>,
    pub identity_files: Vec<IdentityFile>,
    pub proxy_jump: Option<ProxyJump>,
}

/// Something in the config that was ignored, with where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigIssue {
    pub file: PathBuf,
    pub line: LineNo,
    pub reason: IssueReason,
}

/// Why a line or block was ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IssueReason {
    /// A keyword without the value it needs.
    MissingValue { keyword: String },
    /// A value the keyword cannot take (a bad port, a value with control characters).
    BadValue { keyword: String, value: String },
    /// A `"` that is never closed; the line is ignored.
    UnterminatedQuote,
    /// A concrete `Host` name that cannot be an SSH destination.
    UnsafeAlias { alias: String },
    /// A `Match` block: its conditions are not evaluated, so its settings are ignored.
    MatchSkipped,
    /// Includes nested deeper than OpenSSH allows (16), so the rest is ignored.
    IncludeTooDeep,
    /// An included file that exists but cannot be read (a FIFO, a device, an oversize file).
    IncludeUnreadable { path: PathBuf },
    /// Too many include reads or too many bytes for one config (256 files, 1 MiB); the rest is
    /// ignored.
    IncludeBudgetExceeded,
}

/// The result of loading the config: the concrete hosts in file order, and what was ignored.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostList {
    pub entries: Vec<HostEntry>,
    pub issues: Vec<ConfigIssue>,
}
