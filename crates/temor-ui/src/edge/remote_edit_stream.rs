//! Moving a big file of a host to and from the local copy of an edit without holding it whole
//! (PLAN §9b, Tier B). A file that fits in one read is written in one go; a bigger one is read
//! a chunk at a time and written as it arrives, holding every chunk to the same stamp, so a file
//! that changes mid-copy fails instead of landing half old, half new. The way back is the
//! client's `upload`, which streams in parts and applies the write check once, at the end.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use temor_remote::{Client, Error};
use temor_wire::message::{FileStamp, RemotePath};
use temor_wire::units::{ByteCount, ByteOffset, ByteRange};

/// How much of the file one read asks for.
const CHUNK: u64 = 1 << 20;

fn span(offset: u64) -> ByteRange {
    ByteRange {
        offset: ByteOffset(offset),
        len: ByteCount(CHUNK),
    }
}

/// Copies the host's file to `local`, a chunk at a time. Its stamp and length. A file that
/// changes while it is copied is [`Error::ChangedWhileCopying`], and no partial copy is left.
pub fn fetch_to(
    client: &Client,
    path: &RemotePath,
    local: &Path,
) -> Result<(FileStamp, u64), Error> {
    let first = client.read_range(path, span(0))?;
    let (stamp, total) = (first.stamp, first.total.0);
    let io = |error| Error::io(format!("writing {}", local.display()), error);
    let mut file = File::create(local).map_err(io)?;
    let copied = copy_rest(client, path, &mut file, first.bytes, (stamp, total));
    let flushed = copied.and_then(|len| file.flush().map(|()| len).map_err(io));
    match flushed {
        Ok(len) => Ok((stamp, len)),
        Err(error) => {
            drop(file);
            // The partial copy is ours; failing to remove it changes nothing to report.
            let _ = std::fs::remove_file(local);
            Err(error)
        }
    }
}

fn copy_rest(
    client: &Client,
    path: &RemotePath,
    file: &mut File,
    first: Vec<u8>,
    (stamp, total): (FileStamp, u64),
) -> Result<u64, Error> {
    let io = |error| Error::io("writing the local copy".to_owned(), error);
    let mut done = first.len() as u64;
    file.write_all(&first).map_err(io)?;
    while done < total {
        let chunk = client.read_range(path, span(done))?;
        if chunk.stamp != stamp || chunk.bytes.is_empty() {
            return Err(Error::ChangedWhileCopying);
        }
        file.write_all(&chunk.bytes).map_err(io)?;
        done += chunk.bytes.len() as u64;
    }
    Ok(done)
}
