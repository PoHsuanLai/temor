//! Linux: every operation walks from the root's descriptor with `openat2` and
//! `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`, so the kernel refuses a symlink or an escape even if
//! the tree changes between two calls. Folders are made with `mkdirat` relative to a held
//! descriptor, and files are created with `O_EXCL`.

use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;
use std::time::SystemTime;

use rustix::fs::{Mode, OFlags, ResolveFlags, mkdirat, openat2, unlinkat};
use rustix::io::Errno;

use crate::error::TransferError;
use crate::name::RelPath;

/// Folders the transfer makes are 0o755 and files are 0o644. The remote's `prm` is ignored.
const FOLDER_MODE: u32 = 0o755;
const FILE_MODE: u32 = 0o644;

/// Syncs a folder to disk, so a rename made inside it survives a crash.
pub fn sync_folder(path: &Path) -> Result<(), TransferError> {
    let folder = std::fs::File::open(path).map_err(|source| TransferError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    folder.sync_all().map_err(|source| TransferError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve() -> ResolveFlags {
    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS
}

fn io_at(path: &Path, err: Errno) -> TransferError {
    TransferError::Io {
        path: path.to_path_buf(),
        source: io::Error::from(err),
    }
}

/// A link on the way, or an attempt to leave the root, is refused as unsafe. Anything else is an
/// I/O failure.
fn refuse_or_io(path: &Path, err: Errno) -> TransferError {
    match err {
        Errno::LOOP | Errno::XDEV => TransferError::UnsafePath {
            path: path.display().to_string(),
        },
        other => io_at(path, other),
    }
}

/// Splits a path into its folders and its leaf. The root itself has no leaf.
fn split(rel: &RelPath) -> Result<(&[String], &str), TransferError> {
    let (leaf, folders) =
        rel.components()
            .split_last()
            .ok_or_else(|| TransferError::UnsafePath {
                path: String::new(),
            })?;
    Ok((folders, leaf.as_str()))
}

/// The destination folder, held as a descriptor. Every path is resolved from it.
#[derive(Debug)]
pub struct Root {
    fd: OwnedFd,
}

impl Root {
    pub fn open(path: &Path) -> Result<Self, TransferError> {
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        let fd = rustix::fs::open(path, flags, Mode::empty()).map_err(|e| io_at(path, e))?;
        Ok(Self { fd })
    }

    /// Opens `folders` beneath the root, making each one first when `create` is set.
    fn folder(
        &self,
        folders: &[String],
        create: bool,
        at: &Path,
    ) -> Result<OwnedFd, TransferError> {
        let mut current: Option<OwnedFd> = None;
        for part in folders {
            let next = {
                let dir = current.as_ref().map_or(self.fd.as_fd(), AsFd::as_fd);
                if create {
                    match mkdirat(dir, part.as_str(), Mode::from_raw_mode(FOLDER_MODE)) {
                        Ok(()) | Err(Errno::EXIST) => {}
                        Err(err) => return Err(io_at(at, err)),
                    }
                }
                let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
                openat2(dir, part.as_str(), flags, Mode::empty(), resolve())
                    .map_err(|err| refuse_or_io(at, err))?
            };
            current = Some(next);
        }
        match current {
            Some(fd) => Ok(fd),
            None => rustix::io::dup(&self.fd).map_err(|err| io_at(at, err)),
        }
    }

    pub fn make_dir(&self, rel: &RelPath) -> Result<(), TransferError> {
        let path = rel.to_path_buf();
        self.folder(rel.components(), true, &path).map(drop)
    }

    pub fn create_file(&self, rel: &RelPath) -> Result<(), TransferError> {
        let path = rel.to_path_buf();
        let (folders, leaf) = split(rel)?;
        let parent = self.folder(folders, true, &path)?;
        let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC;
        openat2(
            &parent,
            leaf,
            flags,
            Mode::from_raw_mode(FILE_MODE),
            resolve(),
        )
        .map(drop)
        .map_err(|err| refuse_or_io(&path, err))
    }

    /// Opens an existing file for writing, refusing links, and returns it as a `File`.
    fn open_existing(&self, rel: &RelPath, flags: OFlags) -> Result<File, TransferError> {
        let path = rel.to_path_buf();
        let (folders, leaf) = split(rel)?;
        let parent = self.folder(folders, false, &path)?;
        let fd = openat2(
            &parent,
            leaf,
            flags | OFlags::CLOEXEC,
            Mode::empty(),
            resolve(),
        )
        .map_err(|err| refuse_or_io(&path, err))?;
        Ok(File::from(fd))
    }

    pub fn append(&self, rel: &RelPath, data: &[u8]) -> Result<(), TransferError> {
        let path = rel.to_path_buf();
        let mut file = self.open_existing(rel, OFlags::WRONLY | OFlags::APPEND)?;
        file.write_all(data)
            .map_err(|source| TransferError::Io { path, source })
    }

    pub fn finish(&self, rel: &RelPath, modified: Option<SystemTime>) -> Result<(), TransferError> {
        let path = rel.to_path_buf();
        let file = self.open_existing(rel, OFlags::WRONLY)?;
        match modified {
            Some(time) => file
                .set_modified(time)
                .map_err(|source| TransferError::Io { path, source }),
            None => Ok(()),
        }
    }

    /// Removes a file the transfer made. A link is removed as a link, never followed. A missing
    /// file is not an error.
    pub fn discard(&self, rel: &RelPath) -> Result<(), TransferError> {
        let path = rel.to_path_buf();
        let (folders, leaf) = split(rel)?;
        let parent = match self.folder(folders, false, &path) {
            Ok(parent) => parent,
            Err(TransferError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(err) => return Err(err),
        };
        match unlinkat(&parent, leaf, rustix::fs::AtFlags::empty()) {
            Ok(()) | Err(Errno::NOENT) => Ok(()),
            Err(err) => Err(io_at(&path, err)),
        }
    }
}
