//! Viewing a file on another computer (PLAN §10, §9b): ⌘-click or View on a host pane fetches the
//! file into a private copy under the runtime folder and opens the viewer on the copy. Through
//! the host's helper (Tier B) the copy streams in chunks; otherwise `scp` takes it over the
//! shared connection (Tier A). The viewer closing removes the copy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dioxus::prelude::spawn;
use temor_core::host::edit::EditFault;
use temor_core::shelf::PaneSite;
use temor_core::vocab::ids::PaneId;
use temor_ssh::{HostAlias, RemoteFilePath};
use temor_wire::message::RemotePath;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::desk::Desk;
use super::model::{Hub, HubRef};
use super::remote_edit::EditTier;
use super::ssh_edit::{forget_copy, local_name};
use crate::edge::viewer_host::{Cap, CopyEvent, CopyId, CopyOutcome, ToHub};
use crate::edge::viewer_host::{copy_from_helper, copy_with_scp};
use crate::platform::ensure_private_dir;
use crate::session::ViewAsk;

/// A file being fetched for a viewer.
struct Wanted {
    from: PaneId,
    host: HostAlias,
    name: String,
    local: PathBuf,
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
    fn copies_dir(&self) -> PathBuf {
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
        let Some(host) = self.ssh.panes.get(&from).map(|pane| pane.alias.clone()) else {
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
                copy_from_helper(done, conn, (remote, local.clone()), Cap::Unlimited);
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
        self.notices.push(format!("Getting {name}\u{2026}"));
        self.viewers.host.wanted.insert(
            id,
            Wanted {
                from,
                host,
                name,
                local,
            },
        );
    }

    /// A fetch finished: the viewer opens on the copy. A path that is not a file, or is not
    /// there, says nothing: a click on a word that was not a file name is not an error.
    pub(super) fn host_copy_done(&mut self, event: CopyEvent) -> Vec<String> {
        let Some(want) = self.viewers.host.wanted.remove(&event.id) else {
            return Vec::new();
        };
        match event.outcome {
            CopyOutcome::Whole if self.ws.tab_of(want.from).is_some() => {
                self.open_viewer(
                    want.from,
                    ViewAsk {
                        file: want.local,
                        around: Vec::new(),
                        site: PaneSite::Local,
                    },
                );
            }
            CopyOutcome::Failed(fault) => {
                forget_copy(&want.local);
                let words = fault_words(&want.host, &want.name, fault);
                self.notices.push(words);
            }
            // Too big (never asked here), not a file, gone, or the pane closed meanwhile.
            CopyOutcome::Whole
            | CopyOutcome::Header { .. }
            | CopyOutcome::NotAFile
            | CopyOutcome::Gone => forget_copy(&want.local),
        }
        std::mem::take(&mut self.notices)
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
            for note in notes {
                desk.say(note);
            }
            desk.bump();
        }
    });
}
