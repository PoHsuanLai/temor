//! The typed values an OSC 1337 file carries.

use crate::consent::Consent;
use crate::name::FileName;
use crate::units::Bytes;

/// `inline=0` (the default) asks to save the file; `inline=1` asks to show it in the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Delivery {
    Offer,
    Inline,
}

/// `preserveAspectRatio`: the default keeps the image's proportions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// A file the remote offers to save. Its data is private: the only way to reach it is to give
/// the person's consent with [`FileOffer::consent`], which yields a [`Consented`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOffer {
    name: FileName,
    mime: Option<MimeType>,
    data: Vec<u8>,
}

impl FileOffer {
    pub(crate) fn new(name: FileName, mime: Option<MimeType>, data: Vec<u8>) -> Self {
        Self { name, mime, data }
    }

    pub fn name(&self) -> &FileName {
        &self.name
    }

    pub fn mime(&self) -> Option<&MimeType> {
        self.mime.as_ref()
    }

    /// The size of the data the offer carries.
    pub fn size(&self) -> Bytes {
        Bytes::new(self.data.len() as u64)
    }

    /// Applies the person's answer. Only an allowed offer comes back, as a `Consented`, and only
    /// that can be saved. A refused offer is dropped with its data.
    pub fn consent(self, answer: Consent) -> Option<Consented> {
        match answer {
            Consent::Allow => Some(Consented(self)),
            Consent::Refuse => None,
        }
    }
}

/// A file offer the person allowed. Only this type exposes the file's data for saving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consented(FileOffer);

impl Consented {
    pub fn name(&self) -> &FileName {
        self.0.name()
    }

    pub fn mime(&self) -> Option<&MimeType> {
        self.0.mime()
    }

    pub fn data(&self) -> &[u8] {
        &self.0.data
    }
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

/// A complete, decoded OSC 1337 file. An offer waits for consent; an inline image may be shown
/// without it, since showing it writes nothing to disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceivedFile {
    NeedsConsent(FileOffer),
    Inline(InlineImage),
}

/// One decoded OSC 1337 sequence. Multipart pieces stay as base64 text until the file ends.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
