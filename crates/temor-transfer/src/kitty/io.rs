//! Applies a receive machine's writes under the destination root, through the platform edge.
//! The edge resolves every path from the root's descriptor and the kernel refuses symlinks, so a
//! link planted between two writes is refused rather than followed.

use std::io;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::model::{ErrorCode, UnixNanos};
use super::receive::WriteOp;
use crate::Root;
use crate::error::TransferError;

/// Performs one write under `root`. `root` is the transfer's own fresh folder, opened once by the
/// caller.
pub fn apply_write(root: &Root, op: &WriteOp) -> Result<(), TransferError> {
    match op {
        WriteOp::MakeDir(rel) => root.make_dir(rel),
        WriteOp::CreateFile(rel) => root.create_file(rel),
        WriteOp::Append { path, data } => root.append(path, data),
        WriteOp::Finish { path, modified } => root.finish(path, modified.and_then(system_time)),
        WriteOp::Discard(rel) => root.discard(rel),
    }
}

/// The status code to report when a write fails. Anything not recognised is `EIO`.
pub fn wire_code(err: &TransferError) -> ErrorCode {
    match err {
        TransferError::UnsafePath { .. } | TransferError::Unsupported { .. } => ErrorCode::Eperm,
        TransferError::TooLarge { .. }
        | TransferError::TooManyEntries { .. }
        | TransferError::TooManyOpen { .. } => ErrorCode::Efbig,
        TransferError::Io { source, .. } => match source.kind() {
            io::ErrorKind::NotFound => ErrorCode::Enoent,
            io::ErrorKind::AlreadyExists => ErrorCode::Eexist,
            io::ErrorKind::PermissionDenied => ErrorCode::Eperm,
            _ => ErrorCode::Eio,
        },
        _ => ErrorCode::Eio,
    }
}

fn system_time(nanos: UnixNanos) -> Option<SystemTime> {
    let magnitude = Duration::from_nanos(nanos.get().unsigned_abs());
    if nanos.get() >= 0 {
        UNIX_EPOCH.checked_add(magnitude)
    } else {
        UNIX_EPOCH.checked_sub(magnitude)
    }
}
