//! A folder held as a descriptor. Every path is resolved from it with `openat2` and
//! `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`, so the kernel refuses a symlink or an escape even if
//! the tree changes between two calls (the same rule temor-transfer's writer follows).

use std::fs::File;
use std::io::Write;
use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, ResolveFlags, fchmod, fstat, fsync, openat2, renameat,
    statat, unlinkat,
};
use rustix::io::Errno;

use super::stat::raw;
use crate::error::Error;
use crate::platform::RawStat;

const FILE_MODE: u32 = 0o644;
const TEMP_MODE: u32 = 0o600;

fn resolve() -> ResolveFlags {
    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS
}

fn folder_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC
}

/// Maps a failed call on `shown` to the error a caller can act on.
fn map(shown: &str, errno: Errno) -> Error {
    let path = shown.to_string();
    match errno {
        Errno::NOENT => Error::NotFound { path },
        Errno::ACCESS | Errno::PERM | Errno::ROFS => Error::PermissionDenied { path },
        // A link on the way, or a way out of the root.
        Errno::LOOP | Errno::XDEV => Error::Outside { path },
        Errno::ISDIR | Errno::NOTDIR | Errno::NAMETOOLONG => Error::BadPath { path },
        Errno::EXIST => Error::Conflict { current: None },
        other => Error::io(path, other.into()),
    }
}

fn joined(rel: &[String]) -> String {
    if rel.is_empty() {
        ".".to_string()
    } else {
        rel.join("/")
    }
}

/// A folder, and everything reachable beneath it without crossing a link. Holds one descriptor.
#[derive(Debug)]
pub struct Beneath {
    root: OwnedFd,
}

impl Beneath {
    /// Opens the folder. The root itself is opened the ordinary way (it may be reached through a
    /// link); everything below it is not.
    pub fn open(root: &Path) -> Result<Self, Error> {
        let fd = rustix::fs::open(root, folder_flags(), Mode::empty())
            .map_err(|errno| map(&root.display().to_string(), errno))?;
        Ok(Self { root: fd })
    }

    fn open_at(
        &self,
        rel: &[String],
        flags: OFlags,
        mode: Mode,
        shown: &str,
    ) -> Result<OwnedFd, Error> {
        openat2(&self.root, joined(rel), flags, mode, resolve()).map_err(|errno| map(shown, errno))
    }

    /// What `rel` is, without following a link at its end.
    pub fn stat(&self, rel: &[String], shown: &str) -> Result<RawStat, Error> {
        let Some((leaf, folders)) = rel.split_last() else {
            let stat = fstat(&self.root).map_err(|errno| map(shown, errno))?;
            return Ok(raw(&stat));
        };
        let parent = self.open_at(folders, folder_flags(), Mode::empty(), shown)?;
        let stat = statat(&parent, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|errno| map(shown, errno))?;
        Ok(raw(&stat))
    }

    /// The entries of folder `rel` with their state, names that are not UTF-8 left out.
    pub fn read_dir(&self, rel: &[String], shown: &str) -> Result<Vec<(String, RawStat)>, Error> {
        let fd = self.open_at(rel, folder_flags(), Mode::empty(), shown)?;
        let dir = Dir::read_from(&fd).map_err(|errno| map(shown, errno))?;
        let mut entries = Vec::new();
        for entry in dir {
            let entry = entry.map_err(|errno| map(shown, errno))?;
            let Ok(name) = entry.file_name().to_str() else {
                continue;
            };
            if name == "." || name == ".." {
                continue;
            }
            match statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => entries.push((name.to_string(), raw(&stat))),
                // Gone between the listing and the stat.
                Err(Errno::NOENT) => {}
                Err(errno) => return Err(map(shown, errno)),
            }
        }
        Ok(entries)
    }

    /// Opens the regular file `rel` for reading. Anything else (a folder, a pipe) is refused.
    pub fn open_file(&self, rel: &[String], shown: &str) -> Result<(File, RawStat), Error> {
        if rel.is_empty() {
            return Err(Error::BadPath {
                path: shown.to_string(),
            });
        }
        // Non-blocking so that opening a pipe cannot hang the daemon; regular files ignore it.
        let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY;
        let fd = self.open_at(rel, flags, Mode::empty(), shown)?;
        let stat = fstat(&fd).map_err(|errno| map(shown, errno))?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(Error::BadPath {
                path: shown.to_string(),
            });
        }
        Ok((File::from(fd), raw(&stat)))
    }

    /// Replaces file `rel` (or makes it) with `bytes`: written to a new file beside it, synced and
    /// renamed over it, so a reader never sees half of it. An existing file keeps its permissions.
    pub fn replace(
        &self,
        rel: &[String],
        bytes: &[u8],
        nonce: u64,
        shown: &str,
    ) -> Result<(), Error> {
        let (leaf, folders) = rel.split_last().ok_or_else(|| Error::BadPath {
            path: shown.to_string(),
        })?;
        let parent = self.open_at(folders, folder_flags(), Mode::empty(), shown)?;
        let mode = match statat(&parent, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => {
                Mode::from_raw_mode(stat.st_mode)
            }
            Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::Directory => {
                return Err(Error::BadPath {
                    path: shown.to_string(),
                });
            }
            Ok(_) | Err(Errno::NOENT) => Mode::from_raw_mode(FILE_MODE),
            Err(errno) => return Err(map(shown, errno)),
        };
        let temp = format!(".temor-write-{}-{nonce}", std::process::id());
        let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC;
        let fd = openat2(
            &parent,
            temp.as_str(),
            flags,
            Mode::from_raw_mode(TEMP_MODE),
            resolve(),
        )
        .map_err(|errno| map(shown, errno))?;
        let mut file = File::from(fd);
        let written = file
            .write_all(bytes)
            .map_err(|source| Error::io(shown, source))
            .and_then(|()| fchmod(&file, mode).map_err(|errno| map(shown, errno)))
            .and_then(|()| fsync(&file).map_err(|errno| map(shown, errno)))
            .and_then(|()| {
                renameat(&parent, temp.as_str(), &parent, leaf.as_str())
                    .map_err(|errno| map(shown, errno))
            });
        if written.is_err() {
            // The temporary file is ours; a failure to remove it leaves nothing worse behind.
            let _ = unlinkat(&parent, temp.as_str(), AtFlags::empty());
        }
        written
    }

    /// Makes file `rel` with `bytes`, failing with a conflict if anything is there.
    pub fn create_new(&self, rel: &[String], bytes: &[u8], shown: &str) -> Result<(), Error> {
        if rel.is_empty() {
            return Err(Error::BadPath {
                path: shown.to_string(),
            });
        }
        let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC;
        let fd = self.open_at(rel, flags, Mode::from_raw_mode(FILE_MODE), shown)?;
        File::from(fd)
            .write_all(bytes)
            .map_err(|source| Error::io(shown, source))
    }
}
