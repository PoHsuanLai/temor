//! The shelf's files and index on disk. The index is replaced atomically and its folder synced;
//! a file is claimed with `create_new`, so two receipts never share a name. An unreadable index
//! is set aside, never overwritten.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::clock::Clock;
use super::model::Shelf;
use super::naming::candidate_name;
use crate::error::TransferError;
use crate::name::FileName;
use crate::osc1337::Consented;

const INDEX_FILE: &str = "shelf.json";
const INDEX_TEMP: &str = "shelf.json.tmp";
const INDEX_CORRUPT: &str = "shelf.json.corrupt";

/// A folder holding received files and the shelf index. The caller chooses the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfStore {
    dir: PathBuf,
}

impl ShelfStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Reads the index. A missing index is an empty shelf. An unreadable index is moved aside to
    /// `shelf.json.corrupt-<millis>`, so a later save cannot destroy it, and the error is returned.
    pub fn load(&self, clock: &impl Clock) -> Result<Shelf, TransferError> {
        let path = self.dir.join(INDEX_FILE);
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(shelf) => Ok(shelf),
                Err(source) => {
                    self.set_aside(&path, clock)?;
                    Err(TransferError::Index { path, source })
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Shelf::default()),
            Err(source) => Err(TransferError::Io { path, source }),
        }
    }

    /// Renames a corrupt index to a name no existing file holds.
    fn set_aside(&self, path: &Path, clock: &impl Clock) -> Result<(), TransferError> {
        let stamp = clock.now().get();
        let mut n = 0;
        loop {
            let name = match n {
                0 => format!("{INDEX_CORRUPT}-{stamp}"),
                _ => format!("{INDEX_CORRUPT}-{stamp}-{n}"),
            };
            let target = self.dir.join(name);
            if fs::symlink_metadata(&target).is_err() {
                return io_at(path, fs::rename(path, &target));
            }
            n += 1;
        }
    }

    /// Writes the index: a temporary file, flushed, renamed over the old one, then the folder is
    /// synced so the rename itself is durable.
    pub fn save(&self, shelf: &Shelf) -> Result<(), TransferError> {
        let temp = self.dir.join(INDEX_TEMP);
        let target = self.dir.join(INDEX_FILE);
        let bytes = serde_json::to_vec_pretty(shelf).map_err(|source| TransferError::Index {
            path: target.clone(),
            source,
        })?;
        io_at(&temp, fs::create_dir_all(&self.dir))?;
        let mut file = io_at(&temp, File::create(&temp))?;
        io_at(&temp, file.write_all(&bytes))?;
        io_at(&temp, file.sync_all())?;
        drop(file);
        io_at(&target, fs::rename(&temp, &target))?;
        sync_dir(&self.dir)
    }

    /// Moves `source` into the folder under `name`, or the next free name, and returns the final
    /// path. The source is removed only after the copy is complete. If the removal fails, the
    /// copy is taken back out and the error is returned, so no duplicate is left behind.
    pub fn place(&self, source: &Path, name: &FileName) -> Result<PathBuf, TransferError> {
        let placed = self.claim(name, |dest| {
            let mut input = File::open(source)?;
            io::copy(&mut input, dest).map(|_| ())
        })?;
        if let Err(source_err) = fs::remove_file(source) {
            let _ = fs::remove_file(&placed);
            return Err(TransferError::Io {
                path: source.to_path_buf(),
                source: source_err,
            });
        }
        Ok(placed)
    }

    /// Writes a consented offer into the folder under its name, or the next free name. Only a
    /// `Consented` can be passed here, so an offer nobody allowed is never written.
    pub fn place_consented(&self, offer: &Consented) -> Result<PathBuf, TransferError> {
        self.claim(offer.name(), |dest| dest.write_all(offer.data()))
    }

    /// Creates the first free name for `name` and fills it with `write`, syncing the file. A
    /// failed write removes its partial file.
    fn claim(
        &self,
        name: &FileName,
        write: impl FnOnce(&mut File) -> io::Result<()>,
    ) -> Result<PathBuf, TransferError> {
        io_at(&self.dir, fs::create_dir_all(&self.dir))?;
        let mut n = 1;
        loop {
            let candidate = self.dir.join(candidate_name(name.as_str(), n));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut dest) => {
                    if let Err(source) = write(&mut dest).and_then(|()| dest.sync_all()) {
                        drop(dest);
                        let _ = fs::remove_file(&candidate);
                        return Err(TransferError::Io {
                            path: candidate,
                            source,
                        });
                    }
                    return Ok(candidate);
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => n += 1,
                Err(source) => {
                    return Err(TransferError::Io {
                        path: candidate,
                        source,
                    });
                }
            }
        }
    }
}

/// Syncs a folder so a rename within it survives a crash.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> Result<(), TransferError> {
    io_at(dir, File::open(dir).and_then(|folder| folder.sync_all()))
}

// TODO(platform:windows): directories cannot be opened for sync there; the rename is durable
// by the file system's own journal instead.
#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> Result<(), TransferError> {
    Ok(())
}

fn io_at<T>(path: &Path, result: io::Result<T>) -> Result<T, TransferError> {
    result.map_err(|source| TransferError::Io {
        path: path.to_path_buf(),
        source,
    })
}
