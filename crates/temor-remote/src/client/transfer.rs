//! Moving files through the link (PLAN §9b, Tier B): a download in chunks into the downloads
//! folder, an upload streamed in parts, and the pair the edit-locally flow uses: read with a stamp,
//! write back only if the remote file is still that.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use temor_transfer::{RelPath, Root};
use temor_wire::message::{FileStamp, PartEnd, RemotePath, WriteCheck};
use temor_wire::units::{ByteCount, ByteOffset, ByteRange};

use super::calls::ReadChunk;
use super::connect::Client;
use crate::error::Error;

/// How much of a file one request asks for.
const CHUNK: u64 = 1 << 20;

/// How much of a file one part of an upload carries.
const PART: usize = 4 << 20;

fn span(offset: u64) -> ByteRange {
    ByteRange {
        offset: ByteOffset(offset),
        len: ByteCount(CHUNK),
    }
}

impl Client {
    /// Copies a remote file to `rel` under `root`, a chunk at a time. If the remote file changes
    /// while it is copied, or anything fails, nothing is left behind. The stamp of what was
    /// copied.
    pub fn download(
        &self,
        remote: &RemotePath,
        root: &Root,
        rel: &RelPath,
    ) -> Result<FileStamp, Error> {
        let first = self.read_range(remote, span(0))?;
        let stamp = first.stamp;
        root.create_file(rel)?;
        match self.copy_into(remote, root, rel, first) {
            Ok(()) => {
                root.finish(rel, None)?;
                Ok(stamp)
            }
            Err(error) => {
                // The partial file is ours; failing to remove it changes nothing to report.
                let _ = root.discard(rel);
                Err(error)
            }
        }
    }

    fn copy_into(
        &self,
        remote: &RemotePath,
        root: &Root,
        rel: &RelPath,
        first: ReadChunk,
    ) -> Result<(), Error> {
        let (stamp, total) = (first.stamp, first.total.0);
        root.append(rel, &first.bytes)?;
        let mut offset = first.bytes.len() as u64;
        while offset < total {
            let chunk = self.read_range(remote, span(offset))?;
            if chunk.stamp != stamp || chunk.bytes.is_empty() {
                return Err(Error::ChangedWhileCopying);
            }
            root.append(rel, &chunk.bytes)?;
            offset += chunk.bytes.len() as u64;
        }
        Ok(())
    }

    /// The whole of a remote file with its stamp, for editing locally. If the file changes while
    /// it is read, that is [`Error::ChangedWhileCopying`].
    pub fn read_all(&self, remote: &RemotePath) -> Result<(FileStamp, Vec<u8>), Error> {
        let first = self.read_range(remote, span(0))?;
        let (stamp, total) = (first.stamp, first.total.0);
        let mut bytes = first.bytes;
        while (bytes.len() as u64) < total {
            let chunk = self.read_range(remote, span(bytes.len() as u64))?;
            if chunk.stamp != stamp || chunk.bytes.is_empty() {
                return Err(Error::ChangedWhileCopying);
            }
            bytes.extend_from_slice(&chunk.bytes);
        }
        Ok((stamp, bytes))
    }

    /// Writes edited bytes back, only if the remote file is still what `read_all` returned. A
    /// changed file is [`Error::Remote`] holding a conflict, so the person decides.
    pub fn write_back(
        &self,
        remote: &RemotePath,
        seen: FileStamp,
        bytes: &[u8],
    ) -> Result<FileStamp, Error> {
        self.write_file(remote, WriteCheck::Unchanged(seen), bytes)
    }

    /// Copies a local file to the host after `check`. A file that fits in one part goes in one
    /// message; a bigger one streams in parts of 4 MiB, staged beside the target and put in place
    /// when the last has arrived, so `check` is applied once, at the end, and a failure part-way
    /// leaves the target as it was.
    pub fn upload(
        &self,
        local: &Path,
        remote: &RemotePath,
        check: WriteCheck,
    ) -> Result<FileStamp, Error> {
        let fail = |error| Error::io(format!("reading {}", local.display()), error);
        let mut file = File::open(local).map_err(fail)?;
        let mut part = fill(&mut file, PART).map_err(fail)?;
        if part.len() < PART {
            return self.write_file(remote, check, &part);
        }
        let mut offset = 0u64;
        loop {
            // Read ahead, so the part being sent knows whether it is the last.
            let next = match fill(&mut file, PART) {
                Ok(next) => next,
                Err(error) => {
                    self.abort_upload(remote, offset);
                    return Err(fail(error));
                }
            };
            let end = if next.is_empty() {
                PartEnd::Last
            } else {
                PartEnd::More
            };
            match self.write_part(remote, check, ByteOffset(offset), end, &part) {
                Ok(Some(stamp)) => return Ok(stamp),
                Ok(None) => {}
                Err(error) => {
                    self.abort_upload(remote, offset);
                    return Err(error);
                }
            }
            offset += part.len() as u64;
            part = next;
        }
    }

    /// Drops what an upload staged, when it failed part-way. The upload's own error is the one to
    /// report, so a failure here (the link is probably gone) is not.
    fn abort_upload(&self, remote: &RemotePath, offset: u64) {
        let _ = self.write_part(
            remote,
            WriteCheck::Overwrite,
            ByteOffset(offset),
            PartEnd::Abort,
            &[],
        );
    }
}

/// Reads up to `size` bytes, fewer only at the end of the file.
fn fill(file: &mut File, size: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(size);
    file.take(size as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}
