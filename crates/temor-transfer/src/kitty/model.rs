//! Typed values of the kitty protocol. Wire names are snake_case words, read through `Word`.

use ds_core::word::Word;

use crate::error::Malformed;
use crate::units::Bytes;

const MAX_IDENT_BYTES: usize = 128;

/// Checks the protocol's `safe_string` class: `[0-9a-zA-Z_:./@-]`, non-empty, bounded.
fn safe_ident(raw: &str, key: &'static str) -> Result<String, Malformed> {
    let valid = !raw.is_empty()
        && raw.len() <= MAX_IDENT_BYTES
        && raw
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_:./@-".contains(&byte));
    if valid {
        Ok(raw.to_owned())
    } else {
        Err(Malformed::BadValue { key })
    }
}

/// The transfer's id, chosen by the sender.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KittyId(String);

impl KittyId {
    pub fn parse(raw: &str) -> Result<Self, Malformed> {
        safe_ident(raw, "id").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One file or folder within a transfer, chosen by the sender.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(String);

impl FileId {
    pub fn parse(raw: &str) -> Result<Self, Malformed> {
        safe_ident(raw, "fid").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// POSIX permission bits (`prm`). Parsed and carried; applying them needs the platform edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode(pub u32);

/// Modification time (`mod`), in nanoseconds since the UNIX epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixNanos(pub i64);

/// The action of a message (`ac`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum Action {
    /// The remote offers a transfer.
    Send,
    /// Metadata for one file or folder.
    File,
    /// A chunk of one file's data.
    Data,
    /// The last chunk of one file's data.
    EndData,
    /// The remote has finished the transfer.
    Finish,
    /// The remote cancelled the transfer.
    Cancel,
    /// A status message, which only the receiving side sends.
    Status,
    /// Any action this crate does not know. It is answered, not obeyed.
    Unknown,
}

impl Action {
    /// The action named by the wire word. A word outside the set is `Unknown`.
    /// The spec's table says `finish`; its receive example says `finished`. Both are accepted.
    pub fn from_wire(word: &str) -> Self {
        match word {
            "finished" => Self::Finish,
            _ => <Self as Word>::parse(word).unwrap_or(Self::Unknown),
        }
    }
}

/// What a file is (`ft`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum FileType {
    Regular,
    Directory,
    Symlink,
    Link,
}

/// Compression of the data (`zip`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum Compression {
    None,
    Zlib,
}

/// One decoded message. Absent fields are `None`; `data` is empty when there was none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub action: Action,
    pub id: KittyId,
    pub file: Option<FileId>,
    pub parent: Option<FileId>,
    /// The name as sent, decoded from base64 but not yet checked as a path.
    pub name: Option<String>,
    pub file_type: Option<FileType>,
    pub size: Option<Bytes>,
    pub mtime: Option<UnixNanos>,
    pub permissions: Option<Mode>,
    pub compression: Option<Compression>,
    pub data: Vec<u8>,
}

impl Message {
    /// A message with only its action and id set; the other fields are added with struct update.
    pub fn new(action: Action, id: KittyId) -> Self {
        Self {
            action,
            id,
            file: None,
            parent: None,
            name: None,
            file_type: None,
            size: None,
            mtime: None,
            permissions: None,
            compression: None,
            data: Vec::new(),
        }
    }
}

/// The receiving side's status codes, in the protocol's `CODE:message` form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
#[word(case = snake)]
pub enum ErrorCode {
    Eperm,
    Enoent,
    Einval,
    Efbig,
    Eio,
}

impl ErrorCode {
    /// The upper-case code the protocol sends (`EPERM`).
    pub fn code(self) -> String {
        self.slug().to_uppercase()
    }
}

/// A status to send back to the remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyStatus {
    Ok,
    Started,
    Cancelled,
    Failed { code: ErrorCode, message: String },
}

/// One status message to the remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub id: KittyId,
    pub file: Option<FileId>,
    pub status: ReplyStatus,
}
