//! Moving a big file of a host to the local copy of an edit without holding it whole (PLAN §9b,
//! Tier B). The client reads it in the pieces and window it measured for keystroke latency, each
//! held to the same stamp, so a file that changes mid-copy fails instead of landing half old,
//! half new. The way back is the client's `upload`, which streams in parts and applies the write
//! check once, at the end.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use temor_remote::{Client, Error};
use temor_wire::message::{FileStamp, RemotePath};

/// Copies the host's file to `local`. Its stamp and length. A file that changes while it is
/// copied is [`Error::ChangedWhileCopying`], and no partial copy is left.
pub fn fetch_to(
    client: &Client,
    path: &RemotePath,
    local: &Path,
) -> Result<(FileStamp, u64), Error> {
    let io = |error| Error::io(format!("writing {}", local.display()), error);
    let mut file = File::create(local).map_err(io)?;
    let copied = client
        .fetch_into(path, &mut file)
        .and_then(|got| file.flush().map(|()| got).map_err(io));
    if copied.is_err() {
        drop(file);
        // The partial copy is ours; failing to remove it changes nothing to report.
        let _ = std::fs::remove_file(local);
    }
    copied
}
