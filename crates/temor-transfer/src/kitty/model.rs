//! Typed values of the kitty protocol. Wire names are snake_case words: each closed set has
//! `Display` (the word sent) and `FromStr` (the word read).

use crate::error::Malformed;
use crate::units::Bytes;

use super::words::UnknownWord;

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

/// POSIX permission bits (`prm`). Parsed and carried, but ignored by the writer: files are created
/// with mode 0o644 and folders with 0o755, whatever the remote sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode(u32);

impl Mode {
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Modification time (`mod`), in nanoseconds since the UNIX epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixNanos(i64);

impl UnixNanos {
    pub const fn new(nanos: i64) -> Self {
        Self(nanos)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

/// The transmission type (`tt`). The protocol's default is `simple`. `rsync` is a different delta
/// protocol this crate does not implement. Any other word is kept as `Other` so it can be refused
/// with a reply rather than ignored.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Transmission {
    Simple,
    Rsync,
    Other(String),
}

/// The longest transmission word kept. Longer words are malformed.
const MAX_TRANSMISSION_BYTES: usize = 64;

impl Transmission {
    /// Reads the wire word. `None` when the word is longer than the cap; the caller treats that as
    /// malformed.
    pub fn from_wire(word: &str) -> Option<Self> {
        match word {
            "simple" => Some(Self::Simple),
            "rsync" => Some(Self::Rsync),
            other if other.len() <= MAX_TRANSMISSION_BYTES => Some(Self::Other(other.to_owned())),
            _ => None,
        }
    }

    /// The word as it appears on the wire.
    pub fn word(&self) -> &str {
        match self {
            Self::Simple => "simple",
            Self::Rsync => "rsync",
            Self::Other(word) => word,
        }
    }
}

/// Which file-system operation a write was for. The edge reports a failure with this, so the
/// machine knows what, if anything, exists on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WriteKind {
    MakeDir,
    Create,
    Append,
    Finish,
    Discard,
}

/// The action of a message (`ac`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

wire_words!(Action {
    Send => "send",
    File => "file",
    Data => "data",
    EndData => "end_data",
    Finish => "finish",
    Cancel => "cancel",
    Status => "status",
    Unknown => "unknown",
});

impl Action {
    /// The action named by the wire word. A word outside the set is `Unknown`.
    /// The spec's table says `finish`; its receive example says `finished`. Both are accepted.
    pub fn from_wire(word: &str) -> Self {
        match word {
            "finished" => Self::Finish,
            _ => word.parse().unwrap_or(Self::Unknown),
        }
    }
}

/// What a file is (`ft`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Regular,
    Directory,
    Symlink,
    Link,
}

wire_words!(FileType {
    Regular => "regular",
    Directory => "directory",
    Symlink => "symlink",
    Link => "link",
});

/// Compression of the data (`zip`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Compression {
    None,
    Zlib,
}

wire_words!(Compression {
    None => "none",
    Zlib => "zlib",
});

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
    pub transmission: Option<Transmission>,
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
            transmission: None,
            data: Vec::new(),
        }
    }
}

/// The receiving side's status codes, in the protocol's `CODE:message` form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    Eperm,
    Enoent,
    Eexist,
    Einval,
    Efbig,
    Eio,
}

wire_words!(ErrorCode {
    Eperm => "eperm",
    Enoent => "enoent",
    Eexist => "eexist",
    Einval => "einval",
    Efbig => "efbig",
    Eio => "eio",
});

impl ErrorCode {
    /// The upper-case code the protocol sends (`EPERM`).
    pub fn code(self) -> String {
        self.to_string().to_uppercase()
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
