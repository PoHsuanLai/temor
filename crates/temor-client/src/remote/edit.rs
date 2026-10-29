//! Editing a host's file through its helper (PLAN §9b, Tier B): the jobs that read the file with
//! its stamp, look at it again before a write-back, and write it with the check that the file is
//! still the one that was read. They run on a thread each and report as [`SshEvent`]s, the same
//! ones the `scp` jobs send, so the edit's steps are the same for both.

use std::path::PathBuf;
use std::sync::Arc;

use temor_core::host::edit::{EditFault, EditIn, Found};
use temor_remote::Error;
use temor_ssh::{ByteCount, RemoteStat, RemoteState, UnixSeconds};
use temor_wire::message::{EntryKind, FileStamp, RemotePath, WriteCheck};
use tokio::sync::mpsc::UnboundedSender;

use crate::remote::conn::Conn;
use crate::remote::edit_stream::fetch_to;
use crate::ssh::work::{EditId, SshEvent};

/// The host's file as the edit's steps count it: whole seconds and the length.
pub fn stat_of(stamp: FileStamp, len: u64) -> RemoteStat {
    RemoteStat {
        mtime: UnixSeconds(u64::try_from(stamp.mtime.0 / 1000).unwrap_or(0)),
        size: ByteCount(len),
    }
}

/// How a failed call reads to the person.
pub fn fault_of(error: &Error) -> EditFault {
    match error {
        other if other.is_not_found() => EditFault::Gone,
        other if other.is_link_lost() => EditFault::NoConnection,
        _ => EditFault::Refused,
    }
}

/// What a refused write says about the file now: `Some(None)` if it is gone, `Some(Some(stamp))`
/// if it changed, and `None` if the write failed for another reason.
pub fn conflict_of(error: &Error) -> Option<Option<FileStamp>> {
    error.conflict()
}

/// Starts the jobs of edits on a helper. Cloning shares the channel.
#[derive(Debug, Clone)]
pub struct HelperEdits {
    events: UnboundedSender<SshEvent>,
}

impl HelperEdits {
    pub fn new(events: UnboundedSender<SshEvent>) -> Self {
        Self { events }
    }

    fn run(&self, id: EditId, job: impl FnOnce(&Reporter) + Send + 'static) {
        let reporter = Reporter {
            id,
            events: self.events.clone(),
        };
        let started = std::thread::Builder::new()
            .name("temor-helper-edit".to_owned())
            .spawn(move || job(&reporter));
        if started.is_err() {
            let _ = self.events.send(SshEvent::Edit {
                id,
                input: EditIn::Failed(EditFault::NoConnection),
            });
        }
    }

    /// Copies the host's file into `local` (a big one a chunk at a time), then reports
    /// `Fetched`, `FetchGone` or a fault.
    pub fn fetch(&self, id: EditId, conn: Arc<Conn>, path: RemotePath, local: PathBuf) {
        self.run(id, move |report| {
            let input = match fetch_to(conn.client(), &path, &local) {
                Ok((stamp, len)) => {
                    report.seen(stamp);
                    EditIn::Fetched {
                        stat: stat_of(stamp, len),
                    }
                }
                Err(error) => match fault_of(&error) {
                    EditFault::Gone => EditIn::FetchGone,
                    fault => EditIn::Failed(fault),
                },
            };
            report.edit(input);
        });
    }

    /// Reads the host file's time and length.
    pub fn check(&self, id: EditId, conn: Arc<Conn>, path: RemotePath) {
        self.run(id, move |report| {
            let input = match conn.client().stat(&path) {
                Ok(meta) if meta.kind == EntryKind::File => {
                    EditIn::Checked(RemoteState::Present(stat_of(meta.stamp, meta.len.0)))
                }
                Ok(_) => EditIn::Failed(EditFault::Refused),
                Err(error) => match fault_of(&error) {
                    EditFault::Gone => EditIn::Checked(RemoteState::Missing),
                    fault => EditIn::Failed(fault),
                },
            };
            report.edit(input);
        });
    }

    /// Writes `local` over the host's file if it still passes `check` (a big one streams in
    /// parts). A file that changed since it was read is reported as `Conflicted`, and the host's
    /// file is left as it is.
    pub fn store(
        &self,
        id: EditId,
        conn: Arc<Conn>,
        (path, local): (RemotePath, PathBuf),
        check: WriteCheck,
    ) {
        self.run(id, move |report| {
            let Ok(len) = std::fs::metadata(&local).map(|meta| meta.len()) else {
                report.edit(EditIn::Failed(EditFault::Refused));
                return;
            };
            let input = match conn.client().upload(&local, &path, check) {
                Ok(stamp) => {
                    report.seen(stamp);
                    EditIn::Stored {
                        stat: stat_of(stamp, len),
                    }
                }
                Err(error) => match conflict_of(&error) {
                    Some(current) => EditIn::Conflicted(found(&conn, &path, current)),
                    None => EditIn::Failed(fault_of(&error)),
                },
            };
            report.edit(input);
        });
    }
}

/// What the person is told changed: the file's state now, with its length when it can be read.
fn found(conn: &Conn, path: &RemotePath, current: Option<FileStamp>) -> Found {
    match current {
        None => Found::Deleted,
        Some(stamp) => {
            let len = conn.client().stat(path).map_or(0, |meta| meta.len.0);
            Found::Changed(stat_of(stamp, len))
        }
    }
}

/// Where a job sends what it found.
struct Reporter {
    id: EditId,
    events: UnboundedSender<SshEvent>,
}

impl Reporter {
    fn seen(&self, stamp: FileStamp) {
        // The window being gone leaves nobody to tell.
        let _ = self.events.send(SshEvent::EditSeen { id: self.id, stamp });
    }

    fn edit(&self, input: EditIn) {
        let _ = self.events.send(SshEvent::Edit { id: self.id, input });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temor_wire::digest::ContentHash;
    use temor_wire::message::RemoteError;
    use temor_wire::units::UnixMillis;

    fn stamp(millis: i64) -> FileStamp {
        FileStamp {
            mtime: UnixMillis(millis),
            hash: ContentHash([1; 32]),
        }
    }

    fn denied() -> RemoteError {
        RemoteError::PermissionDenied {
            path: RemotePath("/x".to_owned()),
        }
    }

    #[test]
    fn a_stamp_counts_in_whole_seconds_and_never_below_zero() {
        let cases = [(1_999, 1), (2_000, 2), (0, 0), (-5, 0)];
        for (millis, secs) in cases {
            assert_eq!(
                stat_of(stamp(millis), 7).mtime,
                UnixSeconds(secs),
                "{millis}"
            );
        }
        assert_eq!(stat_of(stamp(0), 7).size, ByteCount(7));
    }

    #[test]
    fn a_failed_call_reads_as_gone_lost_or_refused() {
        let missing = || RemotePath("/x".to_owned());
        let cases = [
            (
                "not there",
                Error::NotFound { path: "/x".into() },
                EditFault::Gone,
            ),
            (
                "not there, said by the host",
                Error::Remote(RemoteError::NotFound { path: missing() }),
                EditFault::Gone,
            ),
            (
                "link ended",
                Error::LinkClosed(None),
                EditFault::NoConnection,
            ),
            ("no answer", Error::NoAnswer, EditFault::NoConnection),
            ("permission", Error::Remote(denied()), EditFault::Refused),
            (
                "no file service",
                Error::NotOffered(temor_wire::hello::Capability::Files),
                EditFault::Refused,
            ),
        ];
        for (why, error, want) in cases {
            assert_eq!(fault_of(&error), want, "{why}");
        }
    }

    #[test]
    fn only_a_changed_file_is_a_conflict_and_it_says_what_is_there() {
        let now = stamp(5_000);
        let cases = [
            (
                "changed",
                Error::Remote(RemoteError::Conflict { current: Some(now) }),
                Some(Some(now)),
            ),
            (
                "removed",
                Error::Remote(RemoteError::Conflict { current: None }),
                Some(None),
            ),
            (
                "the local form",
                Error::Conflict { current: Some(now) },
                Some(Some(now)),
            ),
            ("permission", Error::Remote(denied()), None),
            ("link ended", Error::LinkClosed(None), None),
        ];
        for (why, error, want) in cases {
            assert_eq!(conflict_of(&error), want, "{why}");
        }
    }
}
