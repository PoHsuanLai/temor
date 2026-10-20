//! Linux and macOS: every operation walks from the root's descriptor, one name at a time, so a
//! symlink or an escape is refused even if the tree changes between two calls. Linux asks the
//! kernel for that with `openat2` and `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`; macOS has no
//! `openat2`, but a name is a single path component (`RelPath` holds no `/` or `..`), and
//! `openat` with `O_NOFOLLOW` refuses a link as that component. Folders are made with `mkdirat`
//! relative to a held descriptor, and files are created with `O_EXCL`.

use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::SystemTime;

use rustix::fs::{Mode, OFlags, RawMode, mkdirat, symlinkat, unlinkat};
use rustix::io::Errno;

use crate::error::TransferError;
use crate::name::RelPath;

/// Folders the transfer makes start as 0o755 and files as 0o644; the remote's `prm` is applied
/// afterwards (`set_file_mode`, `set_dir_mode`).
const FOLDER_MODE: RawMode = 0o755;
const FILE_MODE: RawMode = 0o644;

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

/// Opens `name`, one path component, in `dir`, refusing a link there. `ELOOP` says a link was
/// refused; on Linux `EXDEV` says the name tried to leave the root.
#[cfg(target_os = "linux")]
fn open_beneath(dir: impl AsFd, name: &str, flags: OFlags, mode: Mode) -> Result<OwnedFd, Errno> {
    use rustix::fs::{ResolveFlags, openat2};
    openat2(
        dir,
        name,
        flags,
        mode,
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
    )
}

#[cfg(target_os = "macos")]
fn open_beneath(dir: impl AsFd, name: &str, flags: OFlags, mode: Mode) -> Result<OwnedFd, Errno> {
    // `name` is one component, so `O_NOFOLLOW` (which refuses a link as the last component) is
    // all of the check. `O_NOFOLLOW_ANY` is no help: macOS answers `EINVAL` when it is combined
    // with `O_CREAT`.
    if name.contains('/') {
        return Err(Errno::INVAL);
    }
    rustix::fs::openat(dir, name, flags | OFlags::NOFOLLOW, mode)
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
                open_beneath(dir, part.as_str(), flags, Mode::empty())
                    .map_err(|err| refuse_or_io(at, err))?
            };
            current = Some(next);
        }
        match current {
            Some(fd) => Ok(fd),
            None => self.fd.try_clone().map_err(|source| TransferError::Io {
                path: at.to_path_buf(),
                source,
            }),
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
        open_beneath(&parent, leaf, flags, Mode::from_raw_mode(FILE_MODE))
            .map(drop)
            .map_err(|err| refuse_or_io(&path, err))
    }

    /// Opens an existing file for writing, refusing links, and returns it as a `File`.
    fn open_existing(&self, rel: &RelPath, flags: OFlags) -> Result<File, TransferError> {
        let path = rel.to_path_buf();
        let (folders, leaf) = split(rel)?;
        let parent = self.folder(folders, false, &path)?;
        let fd = open_beneath(&parent, leaf, flags | OFlags::CLOEXEC, Mode::empty())
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

    /// Sets the permission bits of a file the transfer made, once it is whole. Only the nine
    /// permission bits are applied.
    pub fn set_file_mode(&self, rel: &RelPath, bits: u32) -> Result<(), TransferError> {
        let path = rel.to_path_buf();
        let file = self.open_existing(rel, OFlags::WRONLY)?;
        file.set_permissions(std::fs::Permissions::from_mode(bits & 0o777))
            .map_err(|source| TransferError::Io { path, source })
    }

    /// Sets the permission bits of a folder the transfer made. The owner keeps read, write and
    /// search, so the files that follow can still be put in it.
    pub fn set_dir_mode(&self, rel: &RelPath, bits: u32) -> Result<(), TransferError> {
        let path = rel.to_path_buf();
        let fd = self.folder(rel.components(), false, &path)?;
        File::from(fd)
            .set_permissions(std::fs::Permissions::from_mode((bits & 0o777) | 0o700))
            .map_err(|source| TransferError::Io { path, source })
    }

    /// Makes a symbolic link at `rel` pointing at `target`, which is written as given. The caller
    /// has checked that it stays inside the root; the link is never followed here, and a name
    /// that is taken is refused.
    pub fn make_symlink(&self, rel: &RelPath, target: &str) -> Result<(), TransferError> {
        let path = rel.to_path_buf();
        let (folders, leaf) = split(rel)?;
        let parent = self.folder(folders, true, &path)?;
        symlinkat(target, &parent, leaf).map_err(|err| io_at(&path, err))
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
