//! Editing a host's file locally (PLAN §9b "Remote files"): the file is fetched to a private
//! local copy, opened in the desktop's program for it, and the copy is polled; each settled save
//! goes through `watch_step` and `edit_step`, which check the host's file first and write back,
//! or ask what to do when it changed in between.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use temor_core::host::edit::{
    Edit, EditFault, EditIn, EditOut, LocalStamp, Resolution, Watch, edit_step, watch_step,
};
use temor_core::host::target::{parse_host_path, resolve_in};
use temor_core::vocab::ids::PaneId;
use temor_ssh::{ByteCount, HostAlias, RemoteFilePath};

use super::model::Hub;
use super::remote_edit::{EditTier, Job, Overwrite};
use super::ssh::{Opened, RemoteEdit};
use crate::edge::ssh_work::EditId;
use crate::platform::ensure_private_dir;

/// The name the local copy has: the host file's own name, so the editor picks the right mode.
pub(super) fn local_name(path: &str) -> String {
    let name: String = path
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("file")
        .chars()
        .map(
            |c| match c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                true => c,
                false => '_',
            },
        )
        .collect();
    match name.trim_matches('.') {
        "" => "file".to_owned(),
        _ => name,
    }
}

/// How the local copy looks now; `None` when it cannot be read.
pub(super) fn stamp_of(path: &Path) -> Option<LocalStamp> {
    let meta = std::fs::metadata(path).ok()?;
    let since = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(LocalStamp {
        mtime_nanos: u64::try_from(since.as_nanos()).ok()?,
        size: ByteCount(meta.len()),
    })
}

/// Removes a local copy and the folder made for it. Already gone is as good as removed.
pub(super) fn forget_copy(local: &Path) {
    let _ = std::fs::remove_file(local);
    if let Some(dir) = local.parent() {
        // Only an empty folder goes: the person may have saved something beside the copy.
        let _ = std::fs::remove_dir(dir);
    }
}

fn fault_words(host: &HostAlias, fault: EditFault) -> String {
    let host = host.as_str();
    match fault {
        EditFault::NoConnection => {
            format!("The connection to {host} isn't running; reconnect, then save again")
        }
        EditFault::Gone => format!("The file is no longer on {host}"),
        _ => format!("{host} wouldn't let the file be read or written"),
    }
}

impl Hub {
    /// Opens a host's file in a local editor. `typed` is `host:/path`, or a path on the host of
    /// `pane` (absolute, or relative to the pane's folder).
    pub fn edit_remote(&mut self, pane: Option<PaneId>, typed: &str) {
        let on_pane = pane.and_then(|p| {
            let host = self.ssh.panes.get(&p)?.alias.clone();
            let cwd = self.ws.cwds.get(&p).map(|c| c.as_str().to_owned());
            Some((host, cwd))
        });
        let target = match (parse_host_path(typed), on_pane) {
            (Some(found), _) => Some((found.host, found.path)),
            (None, Some((host, cwd))) => resolve_in(cwd.as_deref(), typed).map(|path| (host, path)),
            (None, None) => None,
        };
        let Some((host, path)) = target else {
            self.notices
                .push("Name the file like computer:/folder/file".to_owned());
            return;
        };
        match self.edit_tier(&host) {
            Ok(tier) => self.begin_edit(host, path, tier),
            // Not connected: connect, then open.
            Err(_) => self.edit_when_up(host, path),
        }
    }

    /// Fetches `path` of `host` to a private local copy and opens it.
    pub(super) fn begin_edit(&mut self, host: HostAlias, path: RemoteFilePath, tier: EditTier) {
        let id = EditId(self.ssh.next_edit);
        self.ssh.next_edit = self.ssh.next_edit.saturating_add(1);
        let dir = self.ssh.env.edit_dir.join(id.0.to_string());
        let made =
            ensure_private_dir(&self.ssh.env.edit_dir).and_then(|()| ensure_private_dir(&dir));
        if let Err(err) = made {
            self.notices
                .push(format!("The local copy can't be made: {err}"));
            return;
        }
        let local = dir.join(local_name(path.as_str()));
        self.ssh.edits.insert(
            id,
            RemoteEdit {
                host,
                path,
                local,
                state: Edit::Opening,
                watch: None,
                opened: Opened::No,
                asking: None,
                tier,
                seen: None,
                overwrite: Overwrite::No,
            },
        );
        self.start_job(id, Job::Fetch);
    }

    /// A step of an edit finished.
    pub(super) fn on_edit(&mut self, id: EditId, input: EditIn) {
        let Some(edit) = self.ssh.edits.get_mut(&id) else {
            return;
        };
        let (state, outs) = edit_step(edit.state, input);
        edit.state = state;
        if matches!(input, EditIn::Fetched { .. }) {
            // The copy was just written: restart the watch so the fetch is not taken for a save.
            edit.watch = stamp_of(&edit.local).map(Watch::new);
        }
        if matches!(input, EditIn::Resolved(_)) {
            edit.asking = None;
        }
        self.carry_out_edit(id, outs);
    }

    fn carry_out_edit(&mut self, id: EditId, outs: Vec<EditOut>) {
        for out in outs {
            let Some(edit) = self.ssh.edits.get_mut(&id) else {
                return;
            };
            let (host, path, local) = (edit.host.clone(), edit.path.clone(), edit.local.clone());
            match out {
                EditOut::Fetch => self.start_job(id, Job::Fetch),
                EditOut::Check => self.start_job(id, Job::Check),
                EditOut::Store => self.start_job(id, Job::Store),
                EditOut::OpenLocal => {
                    if edit.opened == Opened::No {
                        edit.opened = Opened::Yes;
                        if let Err(err) = self.ssh.file_opener.open(&local) {
                            self.notices.push(err.to_string());
                        }
                    }
                }
                EditOut::AskConflict(found) => {
                    edit.asking = Some(found);
                    let panes = self.ssh.panes_on(&host);
                    self.host_attention(panes);
                }
                EditOut::Saved => {
                    self.notices
                        .push(format!("Saved {} to {}", path.as_str(), host.as_str()))
                }
                EditOut::Report(fault) => self.notices.push(fault_words(&host, fault)),
                EditOut::StopWatching => edit.watch = None,
            }
        }
    }

    /// The person chose what to do about a changed file.
    pub fn resolve_edit(&mut self, id: EditId, choice: Resolution) {
        if choice == Resolution::Overwrite
            && let Some(edit) = self.ssh.edits.get_mut(&id)
        {
            edit.overwrite = Overwrite::Yes;
        }
        self.on_edit(id, EditIn::Resolved(choice));
    }

    /// The person is done with the edit; its local copy goes (a save that has not reached the
    /// host yet is the person's to finish first).
    pub fn close_edit(&mut self, id: EditId) {
        self.on_edit(id, EditIn::Closed);
        if let Some(edit) = self.ssh.edits.remove(&id) {
            forget_copy(&edit.local);
        }
    }

    /// Polls the local copies: a settled save goes on to the host.
    pub(super) fn poll_edits(&mut self) -> super::ssh_events::Redraw {
        let saves: Vec<EditId> = self
            .ssh
            .edits
            .iter_mut()
            .filter_map(|(id, edit)| {
                let watch = edit.watch?;
                let (watch, save) = watch_step(watch, stamp_of(&edit.local));
                edit.watch = Some(watch);
                save.map(|_| *id)
            })
            .collect();
        for id in &saves {
            self.on_edit(*id, EditIn::LocalSaved);
        }
        match saves.is_empty() {
            true => super::ssh_events::Redraw::No,
            false => super::ssh_events::Redraw::Yes,
        }
    }

    /// The local copy of an edit, for the person to find.
    pub fn edit_copy(&self, id: EditId) -> Option<PathBuf> {
        self.ssh.edits.get(&id).map(|edit| edit.local.clone())
    }
}
