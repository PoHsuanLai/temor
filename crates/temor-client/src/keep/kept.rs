//! What the window keeps between launches (PLAN §2b, P24b): each Space's rows (`space/<id>`), the
//! Spaces (`spaces`), the hosts (`hosts`) and the history session each pane goes on with
//! (`sessions`), kept in the store's `layout`. A change is handed to the client's `temor-act`
//! thread, and only what changed; at quit it is written before the window goes.
//!
//! The files an older temor wrote (`session.json`, `session-<n>.json`, `spaces.json`) are read
//! into the store once, the first time a window opens on it, and left where they are. A Space
//! kept by a newer temor is read as nothing and never written over.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ds_style::space::list::SpaceId;
use temor_core::vocab::ids::PaneId;
use temor_hosts::host::store::{HostsFile, read_hosts, splice_hosts};
use temor_layout::space::TemorSpaces;
use temor_layout::workspace::restore::{
    FreshWhy, Restored, decode, encode, launch_plan, viewer_plan,
};
use temor_layout::workspace::{KeepClosed, Spawn, ViewerOpen, Workspace, leave_out};
use temor_schema::session::SessionId;

use super::older::{SpacesStartup, Startup, file_name, load_for, load_spaces};
use crate::{Client, Command, Topics};

/// The Spaces, as quire's kit writes them.
const SPACES: &str = "spaces";
/// The hosts: the answers given them and the panes on them.
const HOSTS: &str = "hosts";
/// The history session of each pane, by the pane's number.
const SESSIONS: &str = "sessions";
/// Set once the older files have been read in.
const IMPORTED: &str = "imported/files";
/// How long a write at quit waits for the ones handed to the client's thread before it.
const FLUSH_WITHIN: std::time::Duration = std::time::Duration::from_secs(2);

/// The key of the rows of Space `id`.
fn space_key(id: SpaceId) -> String {
    format!("space/{}", id.0)
}

/// How a write reaches the store: through the client's thread, or now (at quit, when the
/// window may be gone before the thread gets to it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Through {
    Client,
    Now,
}

/// A Space brought back at launch: the panes to start in it and the files its viewers show,
/// in the order they were in.
#[derive(Debug)]
pub struct Starting {
    pub space: SpaceId,
    pub panes: Vec<Spawn>,
    pub viewers: Vec<ViewerOpen>,
}

/// What of the layout the client was last handed, so only a change is handed again.
#[derive(Debug, Default)]
pub struct Kept {
    written: BTreeMap<String, String>,
    /// Keys a newer temor wrote, which this one leaves alone.
    newer: BTreeSet<String>,
    /// What the person is told about what could not be read back.
    notices: Vec<String>,
}

/// Everything [`Kept::lay_all`] keeps.
#[derive(Debug)]
pub struct Laying<'a> {
    pub book: &'a crate::spaces::Book,
    /// Panes that are not brought back (a tab that ran a program for one launch).
    pub leave: &'a BTreeSet<PaneId>,
    pub hosts: &'a HostsFile,
    pub sessions: &'a BTreeMap<PaneId, SessionId>,
}

impl Kept {
    /// What the person is to be told, taken.
    pub fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    /// What is kept under `key`.
    fn kept(client: &Client, key: &str) -> Option<String> {
        client.store().kept(key).ok().flatten()
    }

    /// Keep `value` under `key` (forget it with `None`), unless that is what is kept already.
    fn lay(&mut self, client: &Client, key: String, value: Option<String>, through: Through) {
        if self.newer.contains(&key) || self.written.get(&key) == value.as_ref() {
            return;
        }
        let laid = match through {
            Through::Client => {
                client.act(Command::Lay {
                    key: key.clone(),
                    value: value.clone(),
                });
                true
            }
            Through::Now => write_now(client, &key, value.as_deref()),
        };
        // A write that failed is not what is kept: the next change tries it again.
        if !laid {
            return;
        }
        match value {
            Some(value) => self.written.insert(key, value),
            None => self.written.remove(&key),
        };
    }

    /// Reads the files an older temor left in `dir` into the store, once.
    pub fn import(&mut self, client: &Client, dir: &Path) {
        if Self::kept(client, IMPORTED).is_some() {
            return;
        }
        // The files are marked read only once all they held is in the store: a write that
        // failed is tried again at the next launch.
        let mut all = true;
        let mut ids = vec![SpaceId(0)];
        match load_spaces(dir) {
            Ok(SpacesStartup::Loaded(spaces)) => {
                ids = spaces.list().iter().map(|space| space.id).collect();
                if let Ok(text) = serde_json::to_string(&spaces) {
                    all &= write_now(client, SPACES, Some(&text));
                }
            }
            Ok(SpacesStartup::NoFile) => {}
            Ok(SpacesStartup::Damaged) => self.notices.push(
                "Your Spaces file was damaged. It was set aside and a new one started.".to_owned(),
            ),
            Err(err) => self.notices.push(err.to_string()),
        }
        for id in ids {
            match load_for(dir, id, KeepClosed::DEFAULT) {
                Ok(Startup::Restored(ws)) => {
                    if let Ok(text) = encode(&ws) {
                        all &= write_now(client, &space_key(id), Some(&text));
                    }
                }
                Ok(_) => {}
                Err(err) => self.notices.push(err.to_string()),
            }
        }
        let hosts = std::fs::read_to_string(dir.join(file_name(SpaceId(0))))
            .map(|text| read_hosts(&text))
            .unwrap_or_default();
        if hosts != HostsFile::default()
            && let Ok(text) = splice_hosts("{}", &hosts)
        {
            all &= write_now(client, HOSTS, Some(&text));
        }
        if all {
            write_now(client, IMPORTED, Some("1"));
        }
    }

    /// The Spaces kept; none when nothing usable is.
    fn spaces(&mut self, client: &Client) -> Option<TemorSpaces> {
        let text = Self::kept(client, SPACES)?;
        match serde_json::from_str::<TemorSpaces>(&text) {
            Ok(spaces) if !spaces.list().is_empty() => {
                self.written.insert(SPACES.to_owned(), text);
                Some(spaces)
            }
            _ => {
                self.notices.push(
                    "The Spaces temor kept could not be read. A new one was started.".to_owned(),
                );
                None
            }
        }
    }

    /// The rows kept of Space `id`: none when it has none, or when they could not be used.
    fn space(&mut self, client: &Client, id: SpaceId) -> Option<Workspace> {
        let key = space_key(id);
        let text = Self::kept(client, &key)?;
        match decode(&text, KeepClosed::DEFAULT) {
            Restored::Loaded(ws) => {
                self.written.insert(key, text);
                Some(*ws)
            }
            Restored::Fresh(FreshWhy::NewerVersion(_)) => {
                self.newer.insert(key);
                None
            }
            _ => {
                self.notices.push(
                    "A Space's tabs could not be read back. It starts with a new tab.".to_owned(),
                );
                None
            }
        }
    }

    /// The hosts kept.
    pub fn hosts(&mut self, client: &Client) -> HostsFile {
        let Some(text) = Self::kept(client, HOSTS) else {
            return HostsFile::default();
        };
        self.written.insert(HOSTS.to_owned(), text.clone());
        read_hosts(&text)
    }

    /// The history session each pane brought back goes on with.
    pub fn sessions(&mut self, client: &Client) -> BTreeMap<PaneId, SessionId> {
        let Some(text) = Self::kept(client, SESSIONS) else {
            return BTreeMap::new();
        };
        let read = serde_json::from_str::<BTreeMap<u64, SessionId>>(&text).unwrap_or_default();
        self.written.insert(SESSIONS.to_owned(), text);
        read.into_iter()
            .map(|(pane, id)| (PaneId(pane), id))
            .collect()
    }

    /// Brings every kept Space back into `book`: the Spaces kept (the ones `book` has without a
    /// client), each one's rows, or the place the Spaces recall for a Space with none. Answers
    /// what to start in each. A Space whose rows share a pane with one already brought back is
    /// left out, and the person told.
    pub fn restore(
        &mut self,
        client: Option<&Client>,
        book: &mut crate::spaces::Book,
    ) -> Vec<Starting> {
        if let Some(spaces) = client.and_then(|client| self.spaces(client)) {
            book.begin(spaces);
        }
        let ids: Vec<SpaceId> = book.spaces().list().iter().map(|s| s.id).collect();
        let mut taken = BTreeSet::new();
        let mut starting = Vec::new();
        for id in ids {
            let laid = client.and_then(|client| self.space(client, id));
            let saved = match laid {
                Some(ws) if !ws.tabs.place.tabs.is_empty() => Some(ws),
                _ => book.recalled(id),
            };
            let Some(ws) = saved else { continue };
            let panes = ws.tabs.panes();
            if panes.iter().any(|pane| taken.contains(pane)) {
                self.notices.push(
                    "A saved Space shared a terminal with another and was not restored.".to_owned(),
                );
                continue;
            }
            taken.extend(panes);
            starting.push(Starting {
                space: id,
                panes: launch_plan(&ws),
                viewers: viewer_plan(&ws),
            });
            book.place(id, ws);
        }
        starting
    }

    /// Hands the client every Space's rows, the Spaces, the hosts and the panes' sessions that
    /// changed, and forgets the rows of a Space that is gone.
    pub fn lay_all(&mut self, client: &Client, laying: Laying<'_>, through: Through) {
        // A write still waiting on the client's thread is older than this one: it lands first.
        if through == Through::Now {
            client.flush(FLUSH_WITHIN);
        }
        let rows: Vec<(String, Option<String>)> = laying
            .book
            .every()
            .map(|(id, ws)| (space_key(id), encode(&leave_out(ws, laying.leave)).ok()))
            .collect();
        let kept: BTreeSet<String> = rows.iter().map(|(key, _)| key.clone()).collect();
        let gone: Vec<String> = self
            .written
            .keys()
            .filter(|key| key.starts_with("space/") && !kept.contains(*key))
            .cloned()
            .collect();
        for (key, text) in rows {
            if let Some(text) = text {
                self.lay(client, key, Some(text), through);
            }
        }
        for key in gone {
            self.lay(client, key, None, through);
        }
        let spaces = serde_json::to_string(&laying.book.for_keeping()).ok();
        if let Some(text) = spaces {
            self.lay(client, SPACES.to_owned(), Some(text), through);
        }
        // What could not be written out is left as it was kept, not forgotten.
        let hosts = match *laying.hosts == HostsFile::default() {
            true => Ok(None),
            false => splice_hosts("{}", laying.hosts).map(Some),
        };
        if let Ok(hosts) = hosts {
            self.lay(client, HOSTS.to_owned(), hosts, through);
        }
        let sessions: BTreeMap<u64, SessionId> = laying
            .sessions
            .iter()
            .map(|(pane, id)| (pane.0, *id))
            .collect();
        if let Ok(sessions) = serde_json::to_string(&sessions) {
            self.lay(client, SESSIONS.to_owned(), Some(sessions), through);
        }
    }
}

/// Writes `value` under `key` (forgets it with `None`) on this thread. Whether it was written.
fn write_now(client: &Client, key: &str, value: Option<&str>) -> bool {
    let store = client.store();
    let written = match value {
        Some(value) => store.keep(key, value, Topics::LAYOUT),
        None => store.unkeep(key, Topics::LAYOUT),
    };
    written.is_ok()
}

#[cfg(test)]
#[path = "kept_tests.rs"]
mod tests;
