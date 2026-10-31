//! What the store keeps, table by table: sessions and their commands (history), the pins and the
//! favourites, and the window's own values.

use rusqlite::{OptionalExtension, Row, params};
use temor_schema::ids::BlockId;
use temor_schema::secret::kept_in_history;
use temor_schema::session::{At, RanCommand, Session, SessionId};
use temor_schema::url::{Host, TemorUrl};

use crate::{SqliteStore, StoreError, Topics};

/// A list of places the person keeps, in their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    /// The pinned rows.
    Pins,
    /// The favourites, the tiles above the rows.
    Favourites,
}

impl Shelf {
    fn table(self) -> &'static str {
        match self {
            Shelf::Pins => "pins",
            Shelf::Favourites => "favourites",
        }
    }

    fn topic(self) -> Topics {
        match self {
            Shelf::Pins => Topics::PINS,
            Shelf::Favourites => Topics::FAVOURITES,
        }
    }
}

/// Which commands a history search asks for, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found<'a> {
    /// Text anywhere in the command or its folder, any case.
    pub text: Option<&'a str>,
    /// Only commands run in this folder or under it (an absolute path).
    pub folder: Option<&'a str>,
    /// Only commands started before this, for the next page.
    pub before: Option<At>,
    /// At most this many.
    pub limit: usize,
}

/// The id as SQLite keeps it: its 64 bits, as a signed integer.
fn id_bits(id: SessionId) -> i64 {
    i64::from_ne_bytes(id.bits().to_ne_bytes())
}

fn id_of(bits: i64) -> SessionId {
    SessionId::from_bits(u64::from_ne_bytes(bits.to_ne_bytes()))
}

/// `folder` as a filter: without its trailing `/`, and none for `/`, under which is everything.
fn under(folder: Option<&str>) -> Option<&str> {
    folder
        .map(|f| f.trim_end_matches('/'))
        .filter(|f| !f.is_empty())
}

/// An FTS5 query for `text` as one phrase, so nothing in it is read as syntax. `None` for text
/// shorter than a trigram, which the index cannot find; that is matched by `instr` instead.
fn phrase(text: &str) -> Option<String> {
    (text.chars().count() >= 3).then(|| format!("\"{}\"", text.replace('"', "\"\"")))
}

/// A `sessions` row as read, before its host is.
type SessionColumns = (i64, String, String, Option<String>, i64, Option<i64>);

fn session_row(row: &Row<'_>) -> rusqlite::Result<SessionColumns> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn session_of(
    (id, host, folder, program, started, ended): SessionColumns,
) -> Result<Session, StoreError> {
    let host = Host::parse(&host).map_err(|e| StoreError::Decode {
        what: "sessions.host",
        why: e.to_string(),
    })?;
    Ok(Session {
        id: id_of(id),
        host,
        folder,
        program,
        started: At(started),
        ended: ended.map(At),
    })
}

fn command_of(row: &Row<'_>) -> rusqlite::Result<RanCommand> {
    Ok(RanCommand {
        session: id_of(row.get(0)?),
        block: BlockId(u64::try_from(row.get::<_, i64>(1)?).unwrap_or_default()),
        command: row.get(2)?,
        folder: row.get(3)?,
        exit: row.get(4)?,
        started: At(row.get(5)?),
        ended: row.get::<_, Option<i64>>(6)?.map(At),
    })
}

const SESSION_COLUMNS: &str = "id, host, folder, program, started, ended";
const COMMAND_COLUMNS: &str = "session, block, command, folder, exit, started, ended";
/// A folder filter on `folder`, the path as `?n`: that folder or one under it.
fn folder_clause(n: u8) -> String {
    format!("(?{n} IS NULL OR folder = ?{n} OR substr(folder, 1, length(?{n}) + 1) = ?{n} || '/')")
}

impl SqliteStore {
    /// Keep `session` as it is now: written when it starts, and again when anything about it
    /// changes.
    pub fn save_session(&self, session: &Session) -> Result<(), StoreError> {
        self.touch(Topics::SESSIONS).execute(
            "INSERT INTO sessions (id, host, folder, program, started, ended)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (id) DO UPDATE SET host = excluded.host, folder = excluded.folder,
                program = excluded.program, started = excluded.started, ended = excluded.ended",
            params![
                id_bits(session.id),
                session.host.as_str(),
                session.folder,
                session.program,
                session.started.0,
                session.ended.map(|at| at.0),
            ],
        )?;
        Ok(())
    }

    /// The session ended at `at`.
    pub fn end_session(&self, id: SessionId, at: At) -> Result<(), StoreError> {
        self.touch(Topics::SESSIONS).execute(
            "UPDATE sessions SET ended = ?2 WHERE id = ?1",
            params![id_bits(id), at.0],
        )?;
        Ok(())
    }

    /// One session, if it is kept.
    pub fn session(&self, id: SessionId) -> Result<Option<Session>, StoreError> {
        let row = self
            .reader()
            .query_row(
                &format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE id = ?1"),
                [id_bits(id)],
                session_row,
            )
            .optional()?;
        row.map(session_of).transpose()
    }

    /// Sessions, newest first: those that started in `folder` or under it, before `before`.
    pub fn sessions(
        &self,
        folder: Option<&str>,
        before: Option<At>,
        limit: usize,
    ) -> Result<Vec<Session>, StoreError> {
        let db = self.reader();
        let mut stmt = db.prepare_cached(&format!(
            "SELECT {SESSION_COLUMNS} FROM sessions
             WHERE {} AND (?2 IS NULL OR started < ?2)
             ORDER BY started DESC, id LIMIT ?3",
            folder_clause(1)
        ))?;
        let rows = stmt
            .query_map(
                params![under(folder), before.map(|at| at.0), limit_of(limit)],
                session_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(session_of).collect()
    }

    /// Keep one command a session ran, or what more is known of it (how it ended). One history
    /// may not keep (typed after a space, or holding a secret) is not written, whoever asks.
    pub fn record_command(&self, command: &RanCommand) -> Result<(), StoreError> {
        if !kept_in_history(&command.command) {
            return Ok(());
        }
        self.touch(Topics::BLOCKS).execute(
            "INSERT INTO commands (session, block, command, folder, exit, started, ended)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (session, block) DO UPDATE SET command = excluded.command,
                folder = excluded.folder, exit = excluded.exit, started = excluded.started,
                ended = excluded.ended",
            params![
                id_bits(command.session),
                i64::try_from(command.block.0).unwrap_or(i64::MAX),
                command.command,
                command.folder,
                command.exit,
                command.started.0,
                command.ended.map(|at| at.0),
            ],
        )?;
        Ok(())
    }

    /// A session's commands, in the order it ran them.
    pub fn commands_of(&self, id: SessionId) -> Result<Vec<RanCommand>, StoreError> {
        let db = self.reader();
        let mut stmt = db.prepare_cached(&format!(
            "SELECT {COMMAND_COLUMNS} FROM commands WHERE session = ?1 ORDER BY block"
        ))?;
        Ok(stmt
            .query_map([id_bits(id)], command_of)?
            .collect::<Result<_, _>>()?)
    }

    /// The commands a history search finds, newest first.
    pub fn search(&self, asked: &Found<'_>) -> Result<Vec<RanCommand>, StoreError> {
        let text = asked.text.filter(|t| !t.is_empty());
        let indexed = text.and_then(phrase);
        let short = text.filter(|_| indexed.is_none()).map(str::to_lowercase);
        let db = self.reader();
        let mut stmt = db.prepare_cached(&format!(
            "SELECT {COMMAND_COLUMNS} FROM commands
             WHERE (?1 IS NULL OR rowid IN (SELECT rowid FROM commands_fts WHERE commands_fts MATCH ?1))
               AND (?2 IS NULL OR instr(lower(command), ?2) > 0 OR instr(lower(folder), ?2) > 0)
               AND {}
               AND (?4 IS NULL OR started < ?4)
             ORDER BY started DESC, block DESC LIMIT ?5",
            folder_clause(3)
        ))?;
        Ok(stmt
            .query_map(
                params![
                    indexed,
                    short,
                    under(asked.folder),
                    asked.before.map(|at| at.0),
                    limit_of(asked.limit),
                ],
                command_of,
            )?
            .collect::<Result<_, _>>()?)
    }

    /// Forget what happened before `at`: every command started before it, and every session that
    /// ended before it and has no command left. Returns how many sessions were forgotten. A
    /// session that never said it ended is kept: the client ends those it finds not running.
    pub fn forget_before(&self, at: At) -> Result<usize, StoreError> {
        let db = self.touch(Topics::SESSIONS | Topics::BLOCKS);
        let tx = db.unchecked_transaction()?;
        tx.execute("DELETE FROM commands WHERE started < ?1", [at.0])?;
        let gone = tx.execute(
            "DELETE FROM sessions WHERE ended < ?1
               AND NOT EXISTS (SELECT 1 FROM commands WHERE commands.session = sessions.id)",
            [at.0],
        )?;
        tx.commit()?;
        Ok(gone)
    }

    /// Forget one session and its commands.
    pub fn forget_session(&self, id: SessionId) -> Result<(), StoreError> {
        self.touch(Topics::SESSIONS | Topics::BLOCKS)
            .execute("DELETE FROM sessions WHERE id = ?1", [id_bits(id)])?;
        Ok(())
    }

    /// The places on `shelf`, in order.
    pub fn shelf(&self, shelf: Shelf) -> Result<Vec<TemorUrl>, StoreError> {
        let db = self.reader();
        let mut stmt =
            db.prepare_cached(&format!("SELECT url FROM {} ORDER BY place", shelf.table()))?;
        let urls = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        urls.iter()
            .map(|url| {
                url.parse()
                    .map_err(|e: temor_schema::error::SchemaError| StoreError::Decode {
                        what: "a kept URL",
                        why: e.to_string(),
                    })
            })
            .collect()
    }

    /// Put `places` on `shelf`, in this order, in place of what was there. A place named twice is
    /// kept where it first is.
    pub fn set_shelf(&self, shelf: Shelf, places: &[TemorUrl]) -> Result<(), StoreError> {
        let db = self.touch(shelf.topic());
        let tx = db.unchecked_transaction()?;
        tx.execute(&format!("DELETE FROM {}", shelf.table()), [])?;
        {
            let mut insert = tx.prepare_cached(&format!(
                "INSERT OR IGNORE INTO {} (place, url) VALUES (?1, ?2)",
                shelf.table()
            ))?;
            for (place, url) in places.iter().enumerate() {
                insert.execute(params![limit_of(place), url.to_string()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// A value the window keeps under `key`.
    pub fn kept(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .reader()
            .query_row("SELECT value FROM layout WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// Keep `value` under `key`, saying it touched `topics`.
    pub fn keep(&self, key: &str, value: &str, topics: Topics) -> Result<(), StoreError> {
        self.touch(topics).execute(
            "INSERT INTO layout (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// Forget what is kept under `key`, saying it touched `topics`.
    pub fn unkeep(&self, key: &str, topics: Topics) -> Result<(), StoreError> {
        self.touch(topics)
            .execute("DELETE FROM layout WHERE key = ?1", [key])?;
        Ok(())
    }
}

/// A count as SQLite takes it.
fn limit_of(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

#[cfg(test)]
#[path = "tables_tests.rs"]
mod tests;
