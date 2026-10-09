//! File names and relative paths from the remote side. Both are untrusted: names are cleaned,
//! paths are checked and refused when they could leave the destination folder.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::TransferError;

const MAX_NAME_BYTES: usize = 200;
/// The protocol's limits: each path component at most 255 UTF-8 bytes, the whole path at most 4096.
const MAX_COMPONENT_BYTES: usize = 255;
const MAX_PATH_BYTES: usize = 4096;
const FALLBACK_NAME: &str = "download";

/// A single file name, cleaned so it is safe to create in a folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileName(String);

impl FileName {
    /// Cleans any text into a file name. Separators and control characters become `_`, leading
    /// dots and spaces are dropped (so `..` cannot survive), the length is capped at a character
    /// boundary, and an empty result becomes `download`. Never fails.
    pub fn sanitise(raw: &str) -> Self {
        let mut cleaned = String::new();
        for ch in raw.chars() {
            let ch = if ch == '/' || ch == '\\' || ch.is_control() {
                '_'
            } else {
                ch
            };
            if cleaned.len() + ch.len_utf8() > MAX_NAME_BYTES {
                break;
            }
            cleaned.push(ch);
        }
        let trimmed = cleaned
            .trim_start_matches(|ch: char| ch == '.' || ch.is_whitespace())
            .trim_end();
        if trimmed.is_empty() {
            Self(FALLBACK_NAME.to_owned())
        } else {
            Self(trimmed.to_owned())
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A path relative to a transfer's destination folder. Every component is a real name: no
/// empty, `.`, `..`, separators inside a component, or control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RelPath(Vec<String>);

impl RelPath {
    /// Parses a path the remote sent. Anything that is absolute, has an empty or dot component,
    /// or contains a backslash or control character is refused.
    pub fn parse(raw: &str) -> Result<Self, TransferError> {
        let refuse = || TransferError::UnsafePath {
            path: raw.to_owned(),
        };
        if raw.is_empty() || raw.starts_with('/') || raw.ends_with('/') {
            return Err(refuse());
        }
        if raw.chars().any(|ch| ch == '\\' || ch.is_control()) {
            return Err(refuse());
        }
        if raw.len() > MAX_PATH_BYTES {
            return Err(refuse());
        }
        let parts = raw
            .split('/')
            .map(|part| match part {
                "" | "." | ".." => Err(refuse()),
                _ if part.len() > MAX_COMPONENT_BYTES => Err(refuse()),
                _ => Ok(part.to_owned()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self(parts))
    }

    /// The destination folder itself.
    pub fn root() -> Self {
        Self(Vec::new())
    }

    /// The path of `name` inside this folder.
    pub fn join(&self, name: &RelPath) -> Self {
        Self(self.0.iter().chain(name.0.iter()).cloned().collect())
    }

    pub fn components(&self) -> &[String] {
        &self.0
    }

    /// The path as a platform path, relative. Joining it onto a root is the caller's job.
    pub fn to_path_buf(&self) -> PathBuf {
        self.0.iter().collect()
    }

    /// The last component, which is the file or folder's own name.
    pub fn leaf(&self) -> &str {
        self.0.last().map_or("", String::as_str)
    }
}
