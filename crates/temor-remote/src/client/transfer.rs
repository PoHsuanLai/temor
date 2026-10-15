//! Moving files through the link (PLAN §9b, Tier B): a download in chunks into the downloads
//! folder, an upload streamed in parts, and the pair the edit-locally flow uses: read with a stamp,
//! write back only if the remote file is still that.

use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use temor_transfer::{RelPath, Root};
use temor_wire::message::{FileStamp, PartEnd, RemotePath, WriteCheck};
use temor_wire::units::{ByteCount, ByteOffset, ByteRange};

use super::calls::{InFlight, ReadChunk};
use super::connect::Client;
use crate::error::Error;

/// How much of a file one request asks for.
const CHUNK: u64 = 256 << 10;

/// How much of a file one part of an upload carries.
const PART: usize = 256 << 10;

/// How many reads or parts may be on the line at once. A keystroke waits behind at most this
/// many pieces (half a second at 1 MB/s), and a link holding up to 512 KiB in flight (10 MB/s at
/// 50 ms) is still kept busy; measured in `tests/it/slow_link.rs`.
const WINDOW: usize = 2;

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
        root.append(rel, &first.bytes)?;
        self.read_rest(remote, &first, &mut |bytes| {
            root.append(rel, bytes).map_err(Error::from)
        })
    }

    /// Reads `remote` on from `first` to its end, [`WINDOW`] reads on the line at a time, and
    /// hands each chunk to `each` in order. A chunk that comes back short, or with another stamp
    /// than `first`'s, means the file changed: [`Error::ChangedWhileCopying`].
    fn read_rest(
        &self,
        remote: &RemotePath,
        first: &ReadChunk,
        each: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let (stamp, total) = (first.stamp, first.total.0);
        let mut next = first.bytes.len() as u64;
        if next == 0 && total > 0 {
            return Err(Error::ChangedWhileCopying);
        }
        let mut queue: VecDeque<(InFlight, u64)> = VecDeque::new();
        let result = 'reading: loop {
            while queue.len() < WINDOW && next < total {
                let len = CHUNK.min(total - next);
                let range = ByteRange {
                    offset: ByteOffset(next),
                    len: ByteCount(len),
                };
                match self.send_read(remote, range) {
                    Ok(sent) => queue.push_back((sent, len)),
                    Err(error) => break 'reading Err(error),
                }
                next += len;
            }
            let Some((sent, len)) = queue.pop_front() else {
                break Ok(());
            };
            let chunk = match self.read_answer(sent) {
                Ok(chunk) => chunk,
                Err(error) => break Err(error),
            };
            if chunk.stamp != stamp || chunk.bytes.len() as u64 != len {
                break Err(Error::ChangedWhileCopying);
            }
            if let Err(error) = each(&chunk.bytes) {
                break Err(error);
            }
        };
        for (sent, _) in queue {
            sent.forget(self);
        }
        result
    }

    /// The whole of a remote file with its stamp, for editing locally. If the file changes while
    /// it is read, that is [`Error::ChangedWhileCopying`].
    pub fn read_all(&self, remote: &RemotePath) -> Result<(FileStamp, Vec<u8>), Error> {
        let first = self.read_range(remote, span(0))?;
        let mut bytes = first.bytes.clone();
        self.read_rest(remote, &first, &mut |chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        })?;
        Ok((first.stamp, bytes))
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
    /// message; a bigger one streams in parts of 256 KiB, two on the line at once, staged beside
    /// the target and put in place when the last has arrived, so `check` is applied once, at the
    /// end, and a failure part-way leaves the target as it was.
    pub fn upload(
        &self,
        local: &Path,
        remote: &RemotePath,
        check: WriteCheck,
    ) -> Result<FileStamp, Error> {
        self.upload_with(local, remote, check, &mut |_| {})
    }

    /// [`Client::upload`] that calls `sent` with the bytes each part carried once the host has
    /// taken it, so a caller can show how far the copy got. The calls add up to the file's size
    /// when the upload succeeds; a failed one stops short.
    pub fn upload_with(
        &self,
        local: &Path,
        remote: &RemotePath,
        check: WriteCheck,
        sent: &mut dyn FnMut(u64),
    ) -> Result<FileStamp, Error> {
        let fail = |error| Error::io(format!("reading {}", local.display()), error);
        let mut file = File::open(local).map_err(fail)?;
        let mut part = fill(&mut file, PART).map_err(fail)?;
        if part.len() < PART {
            let stamp = self.write_file(remote, check, &part)?;
            sent(part.len() as u64);
            return Ok(stamp);
        }
        // Up to WINDOW parts are on the line at once; the host takes a connection's parts in
        // the order they were sent.
        let mut offset = 0u64;
        let mut queue: VecDeque<(InFlight, PartEnd, u64)> = VecDeque::new();
        let mut last_sent = false;
        let result = 'sending: loop {
            while queue.len() < WINDOW && !last_sent {
                // Read ahead, so the part being sent knows whether it is the last.
                let next = match fill(&mut file, PART) {
                    Ok(next) => next,
                    Err(error) => break 'sending Err(fail(error)),
                };
                let end = if next.is_empty() {
                    PartEnd::Last
                } else {
                    PartEnd::More
                };
                match self.send_part(remote, check, ByteOffset(offset), end, &part) {
                    Ok(on_line) => queue.push_back((on_line, end, part.len() as u64)),
                    Err(error) => break 'sending Err(error),
                }
                offset += part.len() as u64;
                part = next;
                last_sent = end == PartEnd::Last;
            }
            let Some((on_line, end, len)) = queue.pop_front() else {
                break Err(Error::UnexpectedReply);
            };
            match self.part_answer(on_line, end) {
                Ok(Some(stamp)) => {
                    sent(len);
                    break Ok(stamp);
                }
                Ok(None) => sent(len),
                Err(error) => break Err(error),
            }
        };
        if result.is_err() {
            for (on_line, ..) in queue {
                on_line.forget(self);
            }
            self.abort_upload(remote, offset);
        }
        result
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
