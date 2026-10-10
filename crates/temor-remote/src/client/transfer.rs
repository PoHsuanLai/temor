//! Moving files through the link (PLAN §9b, Tier B): a download in chunks into the downloads
//! folder, an upload in one piece, and the pair the edit-locally flow uses: read with a stamp,
//! write back only if the remote file is still that.

use std::path::Path;

use temor_transfer::{RelPath, Root};
use temor_wire::frame::FrameLimit;
use temor_wire::message::{FileStamp, RemotePath, WriteCheck};
use temor_wire::units::{ByteCount, ByteOffset, ByteRange};

use super::calls::ReadChunk;
use super::connect::Client;
use crate::error::Error;

/// How much of a file one request asks for.
const CHUNK: u64 = 1 << 20;

/// The most an upload may carry: the frame limit less room for the message around it.
fn upload_limit() -> usize {
    (FrameLimit::DEFAULT.0 as usize).saturating_sub(64 * 1024)
}

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

    /// Copies a local file to the host after `check`. The file goes in one message, so it must
    /// fit in one (about 63 MiB); a bigger one is [`Error::TooLarge`].
    pub fn upload(
        &self,
        local: &Path,
        remote: &RemotePath,
        check: WriteCheck,
    ) -> Result<FileStamp, Error> {
        let bytes = std::fs::read(local)
            .map_err(|error| Error::io(format!("reading {}", local.display()), error))?;
        if bytes.len() > upload_limit() {
            return Err(Error::TooLarge);
        }
        self.write_file(remote, check, &bytes)
    }
}
