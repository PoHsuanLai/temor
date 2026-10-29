//! Viewing a file on another computer (PLAN §10, §9b): ⌘-click or View on a host pane fetches the
//! file into a private copy under the runtime folder and opens the viewer on the copy. Through
//! the host's helper (Tier B) the copy streams in chunks; otherwise `scp` takes it over the
//! shared connection (Tier A). The viewer closing removes the copy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dioxus::prelude::spawn;
use temor_client::model::host::edit::EditFault;
use temor_client::model::shelf::PaneSite;
use temor_client::model::vocab::ids::PaneId;
use temor_client::remote::message::RemotePath;
use temor_client::ssh::{HostAlias, RemoteFilePath};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::desk::Desk;
use super::model::{Hub, HubRef};
use super::ssh_edit::{forget_copy, local_name};
use crate::platform::ensure_private_dir;
use crate::session::ViewAsk;
use temor_client::remote::edit_way::EditTier;
use temor_client::viewer_host::{Cap, CopyEvent, CopyId, CopyOutcome, ToHub};
use temor_client::viewer_host::{copy_from_helper, copy_with_scp};

/// What a fetched file is for.
pub(super) enum Then {
    /// A viewer opens on it.
    View,
    /// It is compared with the person's copy `yours` of an edit, and the viewer opens on the
    /// differences.
    Diff { yours: PathBuf },
}

/// A file being fetched for a viewer.
pub(super) struct Wanted {
    pub from: PaneId,
    pub host: HostAlias,
    pub name: String,
    pub local: PathBuf,
    pub then: Then,
}

/// The fetches in flight, and the copies the open viewers show.
pub(super) struct HostCopies {
    next: u64,
    events: UnboundedSender<CopyEvent>,
    inbox: Option<UnboundedReceiver<CopyEvent>>,
    wanted: BTreeMap<CopyId, Wanted>,
    /// The copy each viewer pane shows, removed when the pane ends.
    shown: BTreeMap<PaneId, PathBuf>,
}

impl Default for HostCopies {
    fn default() -> Self {
        let (events, inbox) = unbounded_channel();
        Self {
            next: 1,
            events,
            inbox: Some(inbox),
            wanted: BTreeMap::new(),
            shown: BTreeMap::new(),
        }
    }
}

/// Where the copies of host files are kept: private to the person, beside the edit copies.
pub(super) fn copies_dir(runtime: &Path) -> PathBuf {
    runtime.join("view")
}

/// What the person is told when a file could not be fetched.
fn fault_words(host: &HostAlias, name: &str, fault: EditFault) -> String {
    let host = host.as_str();
    match fault {
        EditFault::NoConnection => {
            format!("The connection to {host} isn't running; reconnect to view {name}")
        }
        _ => format!("{host} wouldn't let {name} be read"),
    }
}

impl Hub {
    pub(super) fn copies_dir(&self) -> PathBuf {
        copies_dir(&self.launch.ambient.runtime_dir)
    }

    /// Whether `file` is one of this window's copies of a host's files.
    pub(super) fn is_host_copy(&self, file: &Path) -> bool {
        file.starts_with(self.copies_dir())
    }

    /// The viewer pane `pane` shows the copy `file`: it is removed when the pane ends.
    pub(super) fn host_copy_shown(&mut self, pane: PaneId, file: &Path) {
        if self.is_host_copy(file) {
            self.viewers.host.shown.insert(pane, file.to_path_buf());
        }
    }

    /// A viewer pane ended: its copy of a host's file goes.
    pub(super) fn host_copy_ended(&mut self, pane: PaneId) {
        if let Some(copy) = self.viewers.host.shown.remove(&pane) {
            forget_copy(&copy);
        }
    }

    /// Fetches `path` of the host `from` is a session on, and opens it in a viewer beside `from`.
    pub fn open_host_viewer(&mut self, from: PaneId, path: &str) {
        let Some(host) = self.alias_of(from) else {
            self.notices
                .push("That file is on a computer that isn't connected".to_owned());
            return;
        };
        let Ok(tier) = self.edit_tier(&host) else {
            self.notices.push(format!(
                "Connect to {} again to view its files",
                host.as_str()
            ));
            return;
        };
        self.begin_fetch((from, host, tier), path, (Then::View, Cap::Unlimited));
    }

    /// Fetches `path` of `host` into a private copy for `then`.
    pub(super) fn begin_fetch(
        &mut self,
        (from, host, tier): (PaneId, HostAlias, EditTier),
        path: &str,
        (then, cap): (Then, Cap),
    ) {
        let id = CopyId(self.viewers.host.next);
        self.viewers.host.next = self.viewers.host.next.saturating_add(1);
        let dir = self.copies_dir();
        let folder = dir.join(id.0.to_string());
        if let Err(err) = ensure_private_dir(&dir).and_then(|()| ensure_private_dir(&folder)) {
            self.notices
                .push(format!("The local copy can't be made: {err}"));
            return;
        }
        let name = local_name(path);
        let local = folder.join(&name);
        let done = Box::new(ToHub {
            id,
            events: self.viewers.host.events.clone(),
        });
        let started = match tier {
            EditTier::Helper => self.remote.helper_conn(&host).map(|conn| {
                let remote = RemotePath(path.to_owned());
                copy_from_helper(done, conn, (remote, local.clone()), cap);
            }),
            EditTier::Scp => RemoteFilePath::new(path).ok().map(|file| {
                let env = self.ssh.env.clone();
                copy_with_scp(done, env, host.clone(), (file, local.clone()));
            }),
        };
        if started.is_none() {
            forget_copy(&local);
            self.notices
                .push(format!("{name} can't be fetched from {}", host.as_str()));
            return;
        }
        if matches!(then, Then::View) {
            self.notices.push(format!("Getting {name}\u{2026}"));
        }
        self.viewers.host.wanted.insert(
            id,
            Wanted {
                from,
                host,
                name,
                local,
                then,
            },
        );
    }

    /// A fetch finished: the viewer opens on the copy. A path that is not a file, or is not
    /// there, says nothing when it was only a click on a word; a comparison says so.
    pub(super) fn host_copy_done(&mut self, event: CopyEvent) -> Vec<String> {
        let Some(want) = self.viewers.host.wanted.remove(&event.id) else {
            return Vec::new();
        };
        let open = self.book.ws.tab_of(want.from).is_some();
        match (event.outcome, &want.then) {
            (CopyOutcome::Whole, Then::View) if open => {
                let file = want.local.clone();
                self.show_file(want.from, file);
            }
            (CopyOutcome::Whole, Then::Diff { yours }) if open => self.show_changes(&want, yours),
            (CopyOutcome::Failed(fault), _) => {
                forget_copy(&want.local);
                let words = fault_words(&want.host, &want.name, fault);
                self.notices.push(words);
            }
            (CopyOutcome::Header { .. }, Then::Diff { .. }) => {
                forget_copy(&want.local);
                self.notices
                    .push(format!("{} is too big to show its changes", want.name));
            }
            (CopyOutcome::Gone, Then::Diff { .. }) => {
                forget_copy(&want.local);
                let host = want.host.as_str();
                self.notices
                    .push(format!("{} is no longer on {host}", want.name));
            }
            // Not a file, gone, or the pane closed meanwhile.
            _ => forget_copy(&want.local),
        }
        std::mem::take(&mut self.notices)
    }

    pub(super) fn show_file(&mut self, from: PaneId, file: PathBuf) {
        self.open_viewer(
            from,
            ViewAsk {
                file,
                around: Vec::new(),
                site: PaneSite::Local,
            },
        );
    }
}

impl HubRef {
    /// See [`Hub::open_host_viewer`].
    pub fn open_host_viewer(&self, from: PaneId, path: &str) -> Vec<String> {
        let mut hub = self.0.borrow_mut();
        hub.open_host_viewer(from, path);
        std::mem::take(&mut hub.notices)
    }
}

/// Hears the fetches of host files for the viewer, for as long as the window lives.
pub fn spawn_viewer_loop(desk: Desk) {
    let Some(mut inbox) = desk.hub.0.borrow_mut().viewers.host.inbox.take() else {
        return;
    };
    spawn(async move {
        while let Some(event) = inbox.recv().await {
            let notes = desk.hub.0.borrow_mut().host_copy_done(event);
            desk.tell(notes);
        }
    });
}
