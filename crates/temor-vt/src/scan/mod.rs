//! The OSC 1337 `File=` pre-scan. Ghostty parses OSC 1337 itself and drops `File=` as
//! unimplemented, so the unknown-sequence callback never sees inline files; the bytes are taken
//! out here, before libghostty, and reported as [`crate::UnknownKind::Osc1337File`].
//!
//! Only `File=` is caught. iTerm2's multipart form (`MultipartFile=`, `FilePart=`, `FileEnd`)
//! still goes to libghostty and is dropped there; catching it belongs with the transfer work
//! (PLAN §9).

#[cfg(feature = "ghostty")]
mod hold;
mod inline_file;
#[cfg(feature = "ghostty")]
mod marks;
#[cfg(feature = "testing")]
mod status;
#[cfg(test)]
mod tests;

#[cfg(feature = "ghostty")]
pub(crate) use hold::{SeqCuts, ends_within};
pub(crate) use inline_file::{InlineFileScanner, Scanned};
#[cfg(feature = "ghostty")]
pub(crate) use marks::MarkCuts;
#[cfg(feature = "testing")]
pub(crate) use status::parse_program_status;
