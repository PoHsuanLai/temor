//! Applies a receive machine's writes under one destination root. Symlinks already on disk are
//! never followed: a path whose existing part crosses a link is refused.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use super::model::UnixNanos;
use super::receive::WriteOp;
use crate::error::TransferError;
use crate::name::RelPath;

/// Performs one write under `root`. `root` is the transfer's own fresh folder, chosen by the caller.
pub fn apply_write(root: &Path, op: &WriteOp) -> Result<(), TransferError> {
    match op {
        WriteOp::MakeDir(rel) => {
            let path = checked(root, rel)?;
            io_at(&path, fs::create_dir_all(&path))
        }
        WriteOp::CreateFile(rel) => {
            let path = checked(root, rel)?;
            if let Some(parent) = path.parent() {
                io_at(&path, fs::create_dir_all(parent))?;
            }
            let file = io_at(
                &path,
                OpenOptions::new().write(true).create_new(true).open(&path),
            )?;
            drop(file);
            Ok(())
        }
        WriteOp::Append { path: rel, data } => {
            let path = checked(root, rel)?;
            let mut file = io_at(&path, OpenOptions::new().append(true).open(&path))?;
            io_at(&path, file.write_all(data))
        }
        WriteOp::Finish {
            path: rel,
            modified,
        } => {
            let path = checked(root, rel)?;
            let file = io_at(&path, File::options().write(true).open(&path))?;
            match modified.and_then(system_time) {
                Some(time) => io_at(&path, file.set_modified(time)),
                None => Ok(()),
            }
        }
        WriteOp::Discard(rel) => {
            let path = checked(root, rel)?;
            match fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(TransferError::Io { path, source: err }),
            }
        }
    }
}

fn io_at<T>(path: &Path, result: io::Result<T>) -> Result<T, TransferError> {
    result.map_err(|source| TransferError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Joins `rel` onto `root`, refusing when any existing component is a symbolic link.
fn checked(root: &Path, rel: &RelPath) -> Result<PathBuf, TransferError> {
    let mut current = root.to_path_buf();
    for part in rel.components() {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(TransferError::UnsafePath {
                    path: current.display().to_string(),
                });
            }
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => break,
            Err(source) => {
                return Err(TransferError::Io {
                    path: current,
                    source,
                });
            }
        }
    }
    Ok(root.join(rel.to_path_buf()))
}

fn system_time(nanos: UnixNanos) -> Option<std::time::SystemTime> {
    let magnitude = Duration::from_nanos(nanos.0.unsigned_abs());
    if nanos.0 >= 0 {
        UNIX_EPOCH.checked_add(magnitude)
    } else {
        UNIX_EPOCH.checked_sub(magnitude)
    }
}
