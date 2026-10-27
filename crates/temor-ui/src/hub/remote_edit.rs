//! A remote edit's calls (PLAN §9b), carried out the way the client's `edit_way` picks: through
//! the host's helper (Tier B) or with `scp` (Tier A). The edit's own steps are temor-core's
//! `edit_step`.

use temor_client::model::host::edit::{EditFault, EditIn};
use temor_client::remote::edit_way::{
    EditTier, HelperUp, NoWay, Overwrite, pick_tier, write_check,
};
use temor_client::remote::message::{FileStamp, RemotePath};
use temor_client::ssh::{HostAlias, RemoteFilePath};

use super::model::Hub;
use temor_client::remote::edit::HelperEdits;
use temor_client::ssh::work::EditId;

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
