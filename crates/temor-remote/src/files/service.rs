//! The file service itself: confined list, stat, read and write.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use temor_wire::digest::ContentHash;
use temor_wire::message::{DirEntry, EntryKind, FileMeta, FileStamp, RemotePath, WriteCheck};
use temor_wire::units::{ByteCount, ByteRange};

use super::scope::Scope;
use super::stamp::{StampCache, sha256};
use super::watch::Observed;
use super::write::{WriteDecision, write_decision};
use crate::error::Error;
use crate::platform::{Beneath, RawStat};
use crate::sync::lock;

/// The most bytes one read returns, so a reply always fits in a frame.
pub const MAX_READ: u64 = 32 << 20;

/// List, stat, read and write inside a [`Scope`]. `Send` and `Sync`: the daemon's connections
/// share one.
#[derive(Debug)]
pub struct FileService {
    scope: Mutex<Scope>,
    cache: StampCache,
    nonce: AtomicU64,
}

/// A path opened for work: the folder it is beneath, its names below it, and how to show it.
pub(super) struct Target {
    pub(super) beneath: Beneath,
    pub(super) rel: Vec<String>,
    pub(super) shown: String,
}

impl FileService {
    /// A service that reaches the folders of `scope`.
    pub fn new(scope: Scope) -> Self {
        Self {
            scope: Mutex::new(scope),
            cache: StampCache::default(),
            nonce: AtomicU64::new(0),
        }
    }

    /// Adds a folder the client may reach (a session's working folder, a relay workspace).
    pub fn grant(&self, root: &Path) -> Result<(), Error> {
        lock(&self.scope).grant(root)
    }

    /// The folders the client may reach now.
    pub(super) fn roots(&self) -> Vec<std::path::PathBuf> {
        lock(&self.scope).roots().to_vec()
    }

    pub(super) fn target(&self, path: &RemotePath) -> Result<Target, Error> {
        let resolved = lock(&self.scope).resolve(path)?;
        Ok(Target {
            beneath: Beneath::open(&resolved.root)?,
            rel: resolved.rel,
            shown: path.0.clone(),
        })
    }

    /// The entries of a folder, by name.
    pub fn list_dir(&self, path: &RemotePath) -> Result<Vec<DirEntry>, Error> {
        let target = self.target(path)?;
        let mut entries: Vec<DirEntry> = target
            .beneath
            .read_dir(&target.rel, &target.shown)?
            .into_iter()
            .map(|(name, stat)| DirEntry {
                name,
                kind: stat.kind,
                len: ByteCount(stat.len),
                mtime: stat.mtime,
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    /// A path's metadata. A regular file's stamp hashes its contents; a folder, a link or anything
    /// else has an all-zero hash, and a link is reported, never followed.
    pub fn stat(&self, path: &RemotePath) -> Result<FileMeta, Error> {
        let target = self.target(path)?;
        self.meta(&target)
    }

    pub(super) fn meta(&self, target: &Target) -> Result<FileMeta, Error> {
        let stat = target.beneath.stat(&target.rel, &target.shown)?;
        if stat.kind != EntryKind::File {
            return Ok(meta_of(&stat, ContentHash([0; 32])));
        }
        let (mut file, opened) = target.beneath.open_file(&target.rel, &target.shown)?;
        let hash = self.cache.hash(&mut file, &opened)?;
        Ok(meta_of(&opened, hash))
    }

    /// The stamp of the regular file at `target`, or `None` if nothing is there.
    pub(super) fn current(&self, target: &Target) -> Result<Option<FileStamp>, Error> {
        match self.meta(target) {
            Ok(meta) if meta.kind == EntryKind::File => Ok(Some(meta.stamp)),
            Ok(_) => Err(Error::BadPath {
                path: target.shown.clone(),
            }),
            Err(Error::NotFound { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// A span of a regular file, at most [`MAX_READ`] bytes: the file's stamp, its full length and
    /// the bytes. A span past the end is empty.
    pub fn read(
        &self,
        path: &RemotePath,
        range: ByteRange,
    ) -> Result<(FileStamp, ByteCount, Vec<u8>), Error> {
        let target = self.target(path)?;
        let (mut file, opened) = target.beneath.open_file(&target.rel, &target.shown)?;
        let hash = self.cache.hash(&mut file, &opened)?;
        let want = range
            .len
            .0
            .min(MAX_READ)
            .min(opened.len.saturating_sub(range.offset.0));
        let mut bytes = Vec::new();
        if want > 0 {
            file.seek(SeekFrom::Start(range.offset.0))
                .map_err(|error| Error::io(&target.shown, error))?;
            file.take(want)
                .read_to_end(&mut bytes)
                .map_err(|error| Error::io(&target.shown, error))?;
        }
        let stamp = FileStamp {
            mtime: opened.mtime,
            hash,
        };
        Ok((stamp, ByteCount(opened.len), bytes))
    }

    /// Writes a file after its [`WriteCheck`]; the new stamp. The file is replaced whole and in
    /// one step. The check and the replace are two steps, so a change made in between is lost
    /// (the same window every editor's save has).
    pub fn write(
        &self,
        path: &RemotePath,
        check: &WriteCheck,
        bytes: &[u8],
    ) -> Result<FileStamp, Error> {
        let target = self.target(path)?;
        let current = self.current(&target)?;
        match write_decision(check, current) {
            WriteDecision::Conflict => return Err(Error::Conflict { current }),
            WriteDecision::Replace => {
                let nonce = self.nonce.fetch_add(1, Ordering::Relaxed);
                target
                    .beneath
                    .replace(&target.rel, bytes, nonce, &target.shown)?;
            }
            WriteDecision::Create => {
                target
                    .beneath
                    .create_new(&target.rel, bytes, &target.shown)
                    .map_err(|error| match error {
                        // Someone made it since the check: say what is there now.
                        Error::Conflict { .. } => Error::Conflict {
                            current: self.meta(&target).ok().map(|meta| meta.stamp),
                        },
                        other => other,
                    })?;
            }
        }
        let stat = target.beneath.stat(&target.rel, &target.shown)?;
        Ok(FileStamp {
            mtime: stat.mtime,
            hash: sha256(bytes),
        })
    }

    /// How a watch sees a path now: `None` when it is not there.
    pub fn observe(&self, path: &RemotePath) -> Result<Option<Observed>, Error> {
        let target = self.target(path)?;
        match target.beneath.stat(&target.rel, &target.shown) {
            Ok(stat) => Ok(Some(Observed {
                kind: stat.kind,
                len: stat.len,
                mtime: stat.mtime,
            })),
            Err(Error::NotFound { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn meta_of(stat: &RawStat, hash: ContentHash) -> FileMeta {
    FileMeta {
        stamp: FileStamp {
            mtime: stat.mtime,
            hash,
        },
        len: ByteCount(stat.len),
        kind: stat.kind,
    }
}
