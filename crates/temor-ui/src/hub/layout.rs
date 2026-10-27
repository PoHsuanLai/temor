//! What the window keeps between launches (PLAN §2b, P24b): each Space's rows (`space/<id>`), the
//! Spaces (`spaces`), the hosts (`hosts`) and the history session each pane goes on with
//! (`sessions`), kept in the store's `layout` through the client. A
//! change is handed to the client's `temor-act` thread as the save pace allows, and only what
//! changed; at quit it is written before the window goes. Without a client nothing is kept.
//!
//! The files an older temor wrote (`session.json`, `session-<n>.json`, `spaces.json`) are read
//! into the store once, the first time a window opens on it, and left where they are. A Space
//! kept by a newer temor is read as nothing and never written over.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ds::components::app::spaces::SpaceId;
use temor_client::{Client, Command, Topics};
use temor_core::host::store::{HostsFile, read_hosts, splice_hosts};
use temor_core::space::TemorSpaces;
use temor_core::vocab::ids::PaneId;
use temor_core::workspace::restore::{FreshWhy, Restored, decode, encode};
use temor_core::workspace::{KeepClosed, Workspace, leave_out};
use temor_schema::session::SessionId;

use super::model::Hub;
use crate::edge::{SpacesStartup, Startup, load_session_for, load_spaces, session_file_name};

/// The Spaces, as quire's kit writes them.
const SPACES: &str = "spaces";
/// The hosts: the answers given them and the panes on them.
const HOSTS: &str = "hosts";
/// The history session of each pane, by the pane's number.
const SESSIONS: &str = "sessions";
/// Set once the older files have been read in.
const IMPORTED: &str = "imported/files";

/// The key of the rows of Space `id`.
fn space_key(id: SpaceId) -> String {
    format!("space/{}", id.0)
}

/// What the window last handed the client, so only a change is handed again.
#[derive(Debug, Default)]
pub(super) struct Laid {
    written: BTreeMap<String, String>,
    /// Keys a newer temor wrote, which this one leaves alone.
    newer: BTreeSet<String>,
}

/// How a write reaches the store: through the client's thread, or now (at quit, when the
/// window may be gone before the thread gets to it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Through {
    Client,
    Now,
}

impl Hub {
    /// What is kept under `key`.
    fn kept(&self, key: &str) -> Option<String> {
        self.client()?.store().kept(key).ok().flatten()
    }

    /// Keep `value` under `key` (forget it with `None`), unless that is what is kept already.
    fn lay(&mut self, key: String, value: Option<String>, through: Through) {
        if self.laid.newer.contains(&key) || self.laid.written.get(&key) == value.as_ref() {
            return;
        }
        let Some(client) = self.client().cloned() else {
            return;
        };
        match &value {
            Some(value) => self.laid.written.insert(key.clone(), value.clone()),
            None => self.laid.written.remove(&key),
        };
        match through {
            Through::Client => client.act(Command::Lay { key, value }),
            Through::Now => write_now(&client, &key, value.as_deref()),
        }
    }

    /// Reads the files an older temor left in `dir` into the store, once.
    pub(super) fn import_files(&mut self, dir: &Path) {
        let Some(client) = self.client().cloned() else {
            return;
        };
        if self.kept(IMPORTED).is_some() {
            return;
        }
        let mut ids = vec![SpaceId(0)];
        match load_spaces(dir) {
            Ok(SpacesStartup::Loaded(spaces)) => {
                ids = spaces.list().iter().map(|space| space.id).collect();
                if let Ok(text) = serde_json::to_string(&spaces) {
                    write_now(&client, SPACES, Some(&text));
                }
            }
            Ok(SpacesStartup::NoFile) => {}
            Ok(SpacesStartup::Damaged) => self.notices.push(
                "Your Spaces file was damaged. It was set aside and a new one started.".to_owned(),
            ),
            Err(err) => self.notices.push(err.to_string()),
        }
        for id in ids {
            match load_session_for(dir, id, KeepClosed::DEFAULT) {
                Ok(Startup::Restored(ws)) => {
                    if let Ok(text) = encode(&ws) {
                        write_now(&client, &space_key(id), Some(&text));
                    }
                }
                Ok(_) => {}
                Err(err) => self.notices.push(err.to_string()),
            }
        }
        let hosts = std::fs::read_to_string(dir.join(session_file_name(SpaceId(0))))
            .map(|text| read_hosts(&text))
            .unwrap_or_default();
        if hosts != HostsFile::default()
            && let Ok(text) = splice_hosts("{}", &hosts)
        {
            write_now(&client, HOSTS, Some(&text));
        }
        write_now(&client, IMPORTED, Some("1"));
    }

    /// The Spaces kept; none when nothing usable is.
    pub(super) fn laid_spaces(&mut self) -> Option<TemorSpaces> {
        let text = self.kept(SPACES)?;
        match serde_json::from_str::<TemorSpaces>(&text) {
            Ok(spaces) if !spaces.list().is_empty() => {
                self.laid.written.insert(SPACES.to_owned(), text);
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
    pub(super) fn laid_space(&mut self, id: SpaceId) -> Option<Workspace> {
        let key = space_key(id);
        let text = self.kept(&key)?;
        match decode(&text, KeepClosed::DEFAULT) {
            Restored::Loaded(ws) => {
                self.laid.written.insert(key, text);
                Some(*ws)
            }
            Restored::Fresh(FreshWhy::NewerVersion(_)) => {
                self.laid.newer.insert(key);
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
    pub(super) fn laid_hosts(&mut self) -> HostsFile {
        let Some(text) = self.kept(HOSTS) else {
            return HostsFile::default();
        };
        self.laid.written.insert(HOSTS.to_owned(), text.clone());
        read_hosts(&text)
    }

    /// The history session each pane brought back goes on with.
    pub(super) fn laid_sessions(&mut self) -> BTreeMap<PaneId, SessionId> {
        let Some(text) = self.kept(SESSIONS) else {
            return BTreeMap::new();
        };
        let read = serde_json::from_str::<BTreeMap<u64, SessionId>>(&text).unwrap_or_default();
        self.laid.written.insert(SESSIONS.to_owned(), text);
        read.into_iter()
            .map(|(pane, id)| (PaneId(pane), id))
            .collect()
    }

    /// Hands the client every Space's rows, the Spaces and the hosts that changed, and forgets
    /// the rows of a Space that is gone.
    pub(super) fn lay_all(&mut self, through: Through) {
        if self.client().is_none() {
            return;
        }
        // A tab that ran a program (`temor -e`) is not brought back: it was that launch's.
        let runs: BTreeSet<PaneId> = self.run.panes.keys().copied().collect();
        let rows: Vec<(String, Option<String>)> = self
            .book
            .parked
            .iter()
            .map(|(id, ws)| (*id, ws))
            .chain([(self.book.active, &self.ws)])
            .map(|(id, ws)| (space_key(id), encode(&leave_out(ws, &runs)).ok()))
            .collect();
        let kept: BTreeSet<String> = rows.iter().map(|(key, _)| key.clone()).collect();
        let gone: Vec<String> = self
            .laid
            .written
            .keys()
            .filter(|key| key.starts_with("space/") && !kept.contains(*key))
            .cloned()
            .collect();
        for (key, text) in rows {
            if let Some(text) = text {
                self.lay(key, Some(text), through);
            }
        }
        for key in gone {
            self.lay(key, None, through);
        }
        let spaces = serde_json::to_string(&self.spaces_for_file()).ok();
        if let Some(text) = spaces {
            self.lay(SPACES.to_owned(), Some(text), through);
        }
        let hosts = self.hosts_file();
        let hosts = match hosts == HostsFile::default() {
            true => None,
            false => splice_hosts("{}", &hosts).ok(),
        };
        self.lay(HOSTS.to_owned(), hosts, through);
        let sessions: BTreeMap<u64, SessionId> = self
            .sessions_to_carry()
            .into_iter()
            .map(|(pane, id)| (pane.0, id))
            .collect();
        let sessions = serde_json::to_string(&sessions).ok();
        self.lay(SESSIONS.to_owned(), sessions, through);
    }
}

/// Writes `value` under `key` (forgets it with `None`) on this thread.
fn write_now(client: &Client, key: &str, value: Option<&str>) {
    let store = client.store();
    let _ = match value {
        Some(value) => store.keep(key, value, Topics::LAYOUT),
        None => store.unkeep(key, Topics::LAYOUT),
    };
}
