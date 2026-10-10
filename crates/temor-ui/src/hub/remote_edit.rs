//! Which way a remote edit reaches the host's file (PLAN §9b): through the helper when the host
//! has one running (Tier B), where the write itself checks that the file is still the one that
//! was read; with `scp` and a look at the file's time and size beforehand otherwise (Tier A).
//! The edit's own steps are temor-core's `edit_step`; these pick the way and carry the jobs out.

use temor_core::host::edit::{EditFault, EditIn};
use temor_core::host::master::Master;
use temor_ssh::{HostAlias, RemoteFilePath};
use temor_wire::message::{FileStamp, RemotePath, WriteCheck};

use super::model::Hub;
use crate::edge::remote_edit::HelperEdits;
use crate::edge::ssh_work::EditId;

/// How an edit's file is read and written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditTier {
    /// `scp` over the shared connection.
    Scp,
    /// The host's helper.
    Helper,
}

/// Whether the host's helper is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HelperUp {
    Yes,
    No,
}

/// Whether the next write goes over a changed file because the person said so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Overwrite {
    Yes,
    No,
}

/// There is no way to reach the host's files: no helper, and no shared connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NoWay;

/// The way to reach a host's files: the helper when it is connected, else `scp` on a shared
/// connection that is up.
pub(super) fn pick_tier(helper: HelperUp, master: Option<&Master>) -> Result<EditTier, NoWay> {
    match (helper, master) {
        (HelperUp::Yes, _) => Ok(EditTier::Helper),
        (HelperUp::No, Some(Master::Up)) => Ok(EditTier::Scp),
        (HelperUp::No, _) => Err(NoWay),
    }
}

/// What a write-back must find on the host. The stamp read last, unless the person chose to
/// overwrite; `None` when there is no stamp to hold the write to (it is never written blind).
pub(super) fn write_check(seen: Option<FileStamp>, overwrite: Overwrite) -> Option<WriteCheck> {
    match (overwrite, seen) {
        (Overwrite::Yes, _) => Some(WriteCheck::Overwrite),
        (Overwrite::No, Some(stamp)) => Some(WriteCheck::Unchanged(stamp)),
        (Overwrite::No, None) => None,
    }
}

/// A call the edit's steps ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Job {
    Fetch,
    Check,
    Store,
}

impl Hub {
    /// The way to reach `host`'s files now.
    pub(super) fn edit_tier(&self, host: &HostAlias) -> Result<EditTier, NoWay> {
        let helper = match self.remote.helper_conn(host) {
            Some(_) => HelperUp::Yes,
            None => HelperUp::No,
        };
        pick_tier(helper, self.ssh.lines.get(host).map(|line| &line.master))
    }

    /// Starts `job` for the edit, the way its host is reached.
    pub(super) fn start_job(&mut self, id: EditId, job: Job) {
        let Some(edit) = self.ssh.edits.get_mut(&id) else {
            return;
        };
        let (host, path, local) = (edit.host.clone(), edit.path.clone(), edit.local.clone());
        let (tier, seen) = (edit.tier, edit.seen);
        // The choice to overwrite is for one write.
        let overwrite = std::mem::replace(&mut edit.overwrite, Overwrite::No);
        match tier {
            EditTier::Scp => match job {
                Job::Fetch => self.ssh.work.fetch(id, host, path, local),
                Job::Check => self.ssh.work.check(id, host, path),
                Job::Store => self.ssh.work.store(id, host, local, path),
            },
            EditTier::Helper => {
                self.start_helper_job(id, job, (host, path, local), (seen, overwrite))
            }
        }
    }

    fn start_helper_job(
        &mut self,
        id: EditId,
        job: Job,
        (host, path, local): (HostAlias, RemoteFilePath, std::path::PathBuf),
        (seen, overwrite): (Option<FileStamp>, Overwrite),
    ) {
        let Some(conn) = self.remote.helper_conn(&host) else {
            self.on_edit(id, EditIn::Failed(EditFault::NoConnection));
            return;
        };
        let jobs = HelperEdits::new(self.ssh.work.sender());
        let remote = RemotePath(path.as_str().to_owned());
        match job {
            Job::Fetch => jobs.fetch(id, conn, remote, local),
            Job::Check => jobs.check(id, conn, remote),
            Job::Store => match write_check(seen, overwrite) {
                Some(check) => jobs.store(id, conn, (remote, local), check),
                None => self.on_edit(id, EditIn::Failed(EditFault::Refused)),
            },
        }
    }

    /// The helper read or wrote the host's file; the next write-back is held to what it saw.
    pub(super) fn on_edit_seen(&mut self, id: EditId, stamp: FileStamp) {
        if let Some(edit) = self.ssh.edits.get_mut(&id) {
            edit.seen = Some(stamp);
        }
    }
}
