//! What can be asked: the store's facts, as values. Each is a fact and nothing more; what a
//! sidebar or the history page makes of it is the window's.

use temor_schema::session::{At, RanCommand, Session, SessionId};
use temor_schema::url::TemorUrl;
use temor_store::{Found, Shelf, SqliteStore};

use crate::{ClientError, Query, Topics};

/// Sessions, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sessions {
    /// Only those that started in this folder or under it (an absolute path).
    pub folder: Option<String>,
    /// Only those that started before this, for the next page.
    pub before: Option<At>,
    /// At most this many.
    pub limit: usize,
}

impl Query for Sessions {
    type Output = Vec<Session>;

    const READS: Topics = Topics::SESSIONS;

    fn run(&self, store: &SqliteStore) -> Result<Vec<Session>, ClientError> {
        Ok(store.sessions(self.folder.as_deref(), self.before, self.limit)?)
    }
}

/// One session and its commands, in the order it ran them; `None` once it is forgotten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Commands(pub SessionId);

impl Query for Commands {
    type Output = Option<(Session, Vec<RanCommand>)>;

    const READS: Topics = Topics::SESSIONS.union(Topics::BLOCKS);

    fn run(&self, store: &SqliteStore) -> Result<Self::Output, ClientError> {
        let Some(session) = store.session(self.0)? else {
            return Ok(None);
        };
        Ok(Some((session, store.commands_of(self.0)?)))
    }
}

/// The commands a history search finds, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// Text anywhere in a command or its folder, any case; none for every command.
    pub text: Option<String>,
    /// Only commands run in this folder or under it (an absolute path).
    pub folder: Option<String>,
    /// Only commands started before this, for the next page.
    pub before: Option<At>,
    /// At most this many.
    pub limit: usize,
}

impl Query for History {
    type Output = Vec<RanCommand>;

    const READS: Topics = Topics::BLOCKS;

    fn run(&self, store: &SqliteStore) -> Result<Vec<RanCommand>, ClientError> {
        Ok(store.search(&Found {
            text: self.text.as_deref(),
            folder: self.folder.as_deref(),
            before: self.before,
            limit: self.limit,
        })?)
    }
}

/// The places on a shelf (the pins, or the favourites), in the person's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnShelf(pub Shelf);

impl Query for OnShelf {
    type Output = Vec<TemorUrl>;

    const READS: Topics = Topics::PINS.union(Topics::FAVOURITES);

    fn run(&self, store: &SqliteStore) -> Result<Vec<TemorUrl>, ClientError> {
        Ok(store.shelf(self.0)?)
    }
}
