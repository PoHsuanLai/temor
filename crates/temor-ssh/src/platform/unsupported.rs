//! Systems without a non-blocking open yet.

use std::fs::File;
use std::io;
use std::path::Path;

/// Without a non-blocking open, only the metadata is checked before opening.
// TODO(platform:macos) and TODO(platform:windows): use the non-blocking open from linux.rs.
pub fn open_regular(path: &Path) -> io::Result<File> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a regular file",
        ));
    }
    File::open(path)
}
