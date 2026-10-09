//! The real filesystem behind [`ConfigFiles`]: regular files only, never a FIFO or a device,
//! and each read is capped. The app uses this at its edge; tests use fixtures.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::include::ConfigFiles;
use crate::platform;

/// The most a single config file may be (256 KiB). Larger files are reported, not read.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;

/// Reads config files from the real filesystem.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsConfigFiles;

impl ConfigFiles for FsConfigFiles {
    fn read(&self, path: &Path) -> io::Result<String> {
        let file = platform::open_regular(path)?;
        let mut text = String::new();
        // One byte past the cap tells an oversize file apart from one exactly at it.
        file.take(MAX_FILE_BYTES + 1).read_to_string(&mut text)?;
        if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "config file too large",
            ));
        }
        Ok(text)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        Ok(entries)
    }
}
