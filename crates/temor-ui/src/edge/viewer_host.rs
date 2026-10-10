//! Getting a file of another computer into a private local copy, for the viewer pane and the
//! peek card (PLAN §10, §9b). Tier B reads it through the host's helper, a chunk at a time, the
//! way a remote edit does (`remote_edit_stream`); Tier A copies it with `scp` over the shared
//! connection. Each copy runs on a thread of its own and reports a [`CopyEvent`].

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use temor_core::host::edit::EditFault;
use temor_remote::{Client, Error};
use temor_ssh::{HostAlias, RemoteFilePath, RemoteFiles, SshRemoteFiles};
use temor_wire::message::{EntryKind, RemotePath};
use temor_wire::units::{ByteCount, ByteOffset, ByteRange};
use tokio::sync::mpsc::UnboundedSender;

use super::SshEnv;
use super::remote_conn::Conn;
use super::remote_edit::fault_of;
use super::remote_edit_stream::fetch_to;
use super::ssh_work::edit_fault;

/// How much of a file too big to copy is read, so its type can still be told.
const HEADER: u64 = 64 * 1024;

/// One copy being made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CopyId(pub u64);

/// How big a file may be and still be copied whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    /// Any size.
    Unlimited,
    /// Up to this many bytes; a bigger file gets its first bytes and its true length.
    Bytes(u64),
}

/// What a copy found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyOutcome {
    /// The whole file is in the local copy.
    Whole,
    /// The file was over the cap: the copy holds its first bytes, and is as long as the file.
    Header { len: u64 },
    /// The path is not a regular file (a folder, a device).
    NotAFile,
    /// The file is not there.
    Gone,
    /// The copy could not be made.
    Failed(EditFault),
}

/// A copy finished.
#[derive(Debug)]
pub struct CopyEvent {
    pub id: CopyId,
    pub outcome: CopyOutcome,
}

/// Where a finished copy is reported: the hub's loop, or the peek's worker.
pub trait CopyDone: Send + 'static {
    fn done(self: Box<Self>, outcome: CopyOutcome);
}

/// Reports to the hub's channel.
#[derive(Debug)]
pub struct ToHub {
    pub id: CopyId,
    pub events: UnboundedSender<CopyEvent>,
}

impl CopyDone for ToHub {
    fn done(self: Box<Self>, outcome: CopyOutcome) {
        // The window being gone leaves nobody to tell.
        let _ = self.events.send(CopyEvent {
            id: self.id,
            outcome,
        });
    }
}

/// Runs `job` on a thread of its own; if none can start, the copy is reported as failed.
fn spawn(done: Box<dyn CopyDone>, job: impl FnOnce() -> CopyOutcome + Send + 'static) {
    let (tell, listen) = std::sync::mpsc::channel::<Box<dyn CopyDone>>();
    let started = std::thread::Builder::new()
        .name("temor-host-copy".to_owned())
        .spawn(move || {
            let outcome = job();
            if let Ok(done) = listen.recv() {
                done.done(outcome);
            }
        });
    match started {
        Ok(_) => {
            // The thread is waiting for this; if it is gone already there is nothing to tell.
            let _ = tell.send(done);
        }
        Err(_) => done.done(CopyOutcome::Failed(EditFault::NoConnection)),
    }
}

/// Copies the helper's file `path` of `conn` to `local`, within `cap`.
pub fn copy_from_helper(
    done: Box<dyn CopyDone>,
    conn: Arc<Conn>,
    (path, local): (RemotePath, PathBuf),
    cap: Cap,
) {
    spawn(done, move || helper_copy(conn.client(), &path, &local, cap));
}

/// Copies `path` of `host` to `local` with `scp` over the shared connection.
pub fn copy_with_scp(
    done: Box<dyn CopyDone>,
    env: SshEnv,
    host: HostAlias,
    (path, local): (RemoteFilePath, PathBuf),
) {
    spawn(done, move || {
        let Ok(layout) = env.layout.as_ref() else {
            return CopyOutcome::Failed(EditFault::NoConnection);
        };
        let files = SshRemoteFiles::new(env.runner.clone(), env.tools.clone(), layout.clone());
        match files.download(&host, &path, &local) {
            Ok(Some(_)) => CopyOutcome::Whole,
            Ok(None) => CopyOutcome::Gone,
            Err(error) => CopyOutcome::Failed(edit_fault(&error)),
        }
    });
}

fn failed(error: &Error) -> CopyOutcome {
    match fault_of(error) {
        EditFault::Gone => CopyOutcome::Gone,
        fault => CopyOutcome::Failed(fault),
    }
}

/// The copy through the helper. A file that is not a regular file is not read at all.
fn helper_copy(client: &Client, path: &RemotePath, local: &Path, cap: Cap) -> CopyOutcome {
    let meta = match client.stat(path) {
        Ok(meta) => meta,
        Err(error) => return failed(&error),
    };
    if meta.kind != EntryKind::File {
        return CopyOutcome::NotAFile;
    }
    match cap {
        Cap::Bytes(most) if meta.len.0 > most => header_copy(client, path, local),
        Cap::Bytes(_) | Cap::Unlimited => match fetch_to(client, path, local) {
            Ok(_) => CopyOutcome::Whole,
            Err(error) => failed(&error),
        },
    }
}

/// The first bytes of a big file, in a local file as long as the original (the rest is a hole),
/// so what looks at it sees the file's type and its size.
fn header_copy(client: &Client, path: &RemotePath, local: &Path) -> CopyOutcome {
    let span = ByteRange {
        offset: ByteOffset(0),
        len: ByteCount(HEADER),
    };
    let chunk = match client.read_range(path, span) {
        Ok(chunk) => chunk,
        Err(error) => return failed(&error),
    };
    let written = File::create(local).and_then(|mut file| {
        std::io::Write::write_all(&mut file, &chunk.bytes)?;
        file.set_len(chunk.total.0)
    });
    match written {
        Ok(()) => CopyOutcome::Header { len: chunk.total.0 },
        Err(_) => {
            // The partial copy is ours; failing to remove it changes nothing to report.
            let _ = std::fs::remove_file(local);
            CopyOutcome::Failed(EditFault::Refused)
        }
    }
}
