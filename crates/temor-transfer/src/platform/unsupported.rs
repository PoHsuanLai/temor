//! Stub for platforms without a kernel-enforced, symlink-refusing resolution yet. Every operation
//! is refused, so a transfer ends cleanly instead of writing through a link.
// TODO(platform:windows): create beneath a directory handle with `NtCreateFile`, refusing reparse
// points, and refuse `:` components (alternate data streams) in `RelPath`.

use std::path::Path;

use crate::error::TransferError;
use crate::name::RelPath;

const FEATURE: &str = "receiving files";

fn unsupported() -> TransferError {
    TransferError::Unsupported { feature: FEATURE }
}

// TODO(platform:windows): directories cannot be opened for sync; the rename is left to the file
// system's own journal.
/// Syncs a folder. Nothing is done here yet, so a rename may not survive a crash on this platform.
pub fn sync_folder(_path: &Path) -> Result<(), TransferError> {
    Ok(())
}

/// The destination folder. On this platform it cannot be opened.
#[derive(Debug)]
pub struct Root(());

impl Root {
    pub fn open(_path: &Path) -> Result<Self, TransferError> {
        Err(unsupported())
    }

    pub fn make_dir(&self, _rel: &RelPath) -> Result<(), TransferError> {
        Err(unsupported())
    }

    pub fn create_file(&self, _rel: &RelPath) -> Result<(), TransferError> {
        Err(unsupported())
    }

    pub fn append(&self, _rel: &RelPath, _data: &[u8]) -> Result<(), TransferError> {
        Err(unsupported())
    }

    pub fn finish(
        &self,
        _rel: &RelPath,
        _modified: Option<std::time::SystemTime>,
    ) -> Result<(), TransferError> {
        Err(unsupported())
    }

    pub fn set_file_mode(&self, _rel: &RelPath, _bits: u32) -> Result<(), TransferError> {
        Err(unsupported())
    }

    pub fn set_dir_mode(&self, _rel: &RelPath, _bits: u32) -> Result<(), TransferError> {
        Err(unsupported())
    }

    pub fn make_symlink(&self, _rel: &RelPath, _target: &str) -> Result<(), TransferError> {
        Err(unsupported())
    }

    pub fn discard(&self, _rel: &RelPath) -> Result<(), TransferError> {
        Err(unsupported())
    }
}
