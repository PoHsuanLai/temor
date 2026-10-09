//! The file reads a sender needs, at the edge.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use super::model::{ChunkEnd, SendFile};
use crate::error::TransferError;
use crate::kitty::model::UnixNanos;
use crate::name::FileName;
use crate::units::Bytes;

/// Looks at a file to send: its cleaned name, its size and its modification time. A folder,
/// a missing file or anything that is not a regular file is an error.
pub fn describe(path: &Path) -> Result<SendFile, TransferError> {
    let io = |source| TransferError::Io {
        path: path.to_path_buf(),
        source,
    };
    let meta = fs::metadata(path).map_err(io)?;
    if !meta.is_file() {
        return Err(TransferError::NotAFile {
            path: path.to_path_buf(),
        });
    }
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_nanos()).ok())
        .map(UnixNanos::new);
    let raw = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(SendFile {
        name: FileName::sanitise(&raw),
        size: Bytes::new(meta.len()),
        modified,
    })
}

/// Reads up to `max` bytes. The chunk is `Last` once the reader has nothing more: a read that
/// fills `max` exactly is `More`, and the next one is then empty and `Last`.
pub fn read_chunk(
    reader: &mut impl Read,
    max: Bytes,
    path: &Path,
) -> Result<(Vec<u8>, ChunkEnd), TransferError> {
    let want = usize::try_from(max.get())
        .unwrap_or(usize::MAX)
        .min(1 << 20);
    let mut data = vec![0_u8; want];
    let mut filled = 0;
    while filled < want {
        let read = reader
            .read(&mut data[filled..])
            .map_err(|source| TransferError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            data.truncate(filled);
            return Ok((data, ChunkEnd::Last));
        }
        filled += read;
    }
    Ok((data, ChunkEnd::More))
}
