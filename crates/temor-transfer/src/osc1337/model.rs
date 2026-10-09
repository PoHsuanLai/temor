//! The typed values an OSC 1337 file carries.

use ds_core::word::Word;

use crate::name::FileName;
use crate::units::Bytes;

/// Whether the terminal's own limit cut the sequence short. The app maps temor-vt's truncation
/// onto this when it hands a payload over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Completeness {
    Whole,
    Cut,
}

/// `inline=0` (the default) asks to save the file; `inline=1` asks to show it in the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Delivery {
    Offer,
    Inline,
}

/// `preserveAspectRatio`: the default keeps the image's proportions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Aspect {
    Preserve,
    Stretch,
}

/// A `width` or `height`: cells, pixels, a percentage of the pane, or the image's own size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    Auto,
    Cells(u32),
    Pixels(u32),
    Percent(u32),
}

/// A MIME type from the `type` argument, restricted to plain ASCII token characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MimeType(String);

impl MimeType {
    /// Accepts `type/subtype`-shaped ASCII text only. Anything else is refused by the caller.
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let valid = !raw.is_empty()
            && raw.len() <= 255
            && raw.contains('/')
            && raw
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/+-.".contains(&byte));
        valid.then(|| Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The arguments of a `File=` (or `MultipartFile=`) header, without the data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHeader {
    pub name: FileName,
    pub declared_size: Option<Bytes>,
    pub mime: Option<MimeType>,
    pub width: Dimension,
    pub height: Dimension,
    pub aspect: Aspect,
    pub delivery: Delivery,
}

impl FileHeader {
    pub(crate) fn new() -> Self {
        Self {
            name: FileName::sanitise(""),
            declared_size: None,
            mime: None,
            width: Dimension::Auto,
            height: Dimension::Auto,
            aspect: Aspect::Preserve,
            delivery: Delivery::Offer,
        }
    }
}

/// A file the remote offers to save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOffer {
    pub name: FileName,
    pub mime: Option<MimeType>,
    pub data: Vec<u8>,
}

/// An image the remote asks to show in the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineImage {
    pub name: FileName,
    pub mime: Option<MimeType>,
    pub width: Dimension,
    pub height: Dimension,
    pub aspect: Aspect,
    pub data: Vec<u8>,
}

/// A complete, decoded OSC 1337 file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceivedFile {
    Offer(FileOffer),
    Inline(InlineImage),
}

/// One decoded OSC 1337 sequence. Multipart pieces stay as base64 text until the file ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Osc1337Command {
    /// A whole `File=` sequence: its header and the decoded data.
    Whole { header: FileHeader, data: Vec<u8> },
    /// `MultipartFile=`: the header of a file that arrives in parts.
    MultipartStart(FileHeader),
    /// `FilePart=`: one piece of base64 text.
    Part(Vec<u8>),
    /// `FileEnd`.
    End,
}
