//! The shelf's files and index on disk. The index is replaced atomically; a file is claimed with
//! `create_new`, so two receipts never share a name.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use super::model::Shelf;
use super::naming::candidate_name;
use crate::error::TransferError;
use crate::name::FileName;

const INDEX_FILE: &str = "shelf.json";
const INDEX_TEMP: &str = "shelf.json.tmp";

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

    /// Reads the index. A missing index is an empty shelf; an unreadable one is an error.
    pub fn load(&self) -> Result<Shelf, TransferError> {
        let path = self.dir.join(INDEX_FILE);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|source| TransferError::Index { path, source }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Shelf::default()),
            Err(source) => Err(TransferError::Io { path, source }),
        }
    }

    /// Writes the index: a temporary file, flushed, then renamed over the old one.
    pub fn save(&self, shelf: &Shelf) -> Result<(), TransferError> {
        let temp = self.dir.join(INDEX_TEMP);
        let target = self.dir.join(INDEX_FILE);
        let bytes = serde_json::to_vec_pretty(shelf).map_err(|source| TransferError::Index {
            path: target.clone(),
            source,
        })?;
        io_at(&temp, fs::create_dir_all(&self.dir))?;
        let mut file = io_at(&temp, File::create(&temp))?;
        io_at(&temp, io::Write::write_all(&mut file, &bytes))?;
        io_at(&temp, file.sync_all())?;
        drop(file);
        io_at(&target, fs::rename(&temp, &target))
    }

    /// Moves `source` into the folder under `name`, or the next free name, and returns the final
    /// path. The source is removed only after the copy is complete.
    pub fn place(&self, source: &Path, name: &FileName) -> Result<PathBuf, TransferError> {
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
                    let copied = copy_into(source, &mut dest);
                    if let Err(err) = copied {
                        drop(dest);
                        let _ = fs::remove_file(&candidate);
                        return Err(err);
                    }
                    io_at(source, fs::remove_file(source))?;
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

fn copy_into(source: &Path, dest: &mut File) -> Result<(), TransferError> {
    let mut input = io_at(source, File::open(source))?;
    io_at(source, io::copy(&mut input, dest).map(|_| ()))?;
    io_at(source, dest.sync_all())
}

fn io_at<T>(path: &Path, result: io::Result<T>) -> Result<T, TransferError> {
    result.map_err(|source| TransferError::Io {
        path: path.to_path_buf(),
        source,
    })
}
