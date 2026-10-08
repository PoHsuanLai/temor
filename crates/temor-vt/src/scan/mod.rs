//! The OSC 1337 `File=` pre-scan. Ghostty parses OSC 1337 itself and drops `File=` as
//! unimplemented, so the unknown-sequence callback never sees inline files; the bytes are taken
//! out here, before libghostty, and reported as [`crate::UnknownKind::Osc1337File`].

mod inline_file;
#[cfg(test)]
mod tests;

pub(crate) use inline_file::{InlineFileScanner, Scanned};
