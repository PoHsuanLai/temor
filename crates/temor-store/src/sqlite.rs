//! The store: one SQLite connection that writes, and read-only ones beside it. Ported from
//! mailo's `mail-store` (`sqlite/mod.rs`), less what is mail's.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use rusqlite::Connection;

use crate::changes::{Changes, Committed, Generation, Topics};
use crate::{StoreError, migrate};

/// temor's database: history, pins, favourites and the window's layout.
///
/// One writer behind a reentrant lock (a write method takes it, then helpers take it again on the
/// same thread), and up to [`READERS`] read-only connections, so a read waits behind a commit
/// only while SQLite itself would make it.
#[derive(Debug)]
pub struct SqliteStore {
    /// Behind a mutex because `rusqlite::Connection` is `Send` but not `Sync`. Reentrant because
    /// a write opens a transaction and then calls helpers that need the connection again; a
    /// plain mutex there is a deadlock with no message.
    db: ReentrantMutex<Connection>,
    /// Connections that only read. Empty for an in-memory database, which a second connection
    /// cannot share; reads there go to the writer.
    readers: Vec<ReentrantMutex<Connection>>,
    /// Who hears a commit, and what the writer has done under its current hold.
    changes: Changes,
    /// The database's file. `None` in memory.
    file: Option<PathBuf>,
}

/// Make the database at `path` its owner's alone (0600) before SQLite opens it, with its `-wal`
/// and `-shm`: history holds every command the person typed.
fn private_files(path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .map_err(StoreError::db)?;
        for suffix in ["", "-wal", "-shm"] {
            let mut file = path.as_os_str().to_owned();
            file.push(suffix);
            match std::fs::set_permissions(
                PathBuf::from(file),
                std::fs::Permissions::from_mode(0o600),
            ) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(StoreError::db(e));
                }
                _ => {}
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// How many read-only connections to open beside the writer: the sidebar, the history page, and
/// one for whatever else asks.
const READERS: usize = 3;

impl SqliteStore {
    /// Open the database at `path`, making it (and its folder's file) if it is new.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        private_files(&path)?;
        let mut store = Self::from_connection(Connection::open(&path)?)?;
        store.file = Some(path.clone());
        // After the migrations: a reader opened on an unmigrated file would have no tables.
        for _ in 0..READERS {
            let reader = Connection::open(&path)?;
            reader.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA query_only = ON;",
            )?;
            store.readers.push(ReentrantMutex::new(reader));
        }
        Ok(store)
    }

    /// An in-memory database, gone when dropped. For tests.
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(db: Connection) -> Result<Self, StoreError> {
        // WAL lets a reader run while the writer commits. NORMAL skips a fsync per commit at the
        // risk of the last one on power loss, which for history is a command or two. Two writers
        // (a second window process) wait for each other up to five seconds.
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )?;
        migrate::migrate(&db)?;
        // After the migrations, which no one is listening to. SQLite calls the hook only for a
        // transaction that wrote, before the commit is made; what it marks is said when the
        // writer is let go of, by which time readers can see it.
        let changes = Changes::default();
        let committed = changes.commit_flag();
        db.commit_hook(Some(move || {
            committed.store(true, Ordering::Relaxed);
            // `false` lets the commit go ahead.
            false
        }))?;
        Ok(Self {
            db: ReentrantMutex::new(db),
            readers: Vec::new(),
            changes,
            file: None,
        })
    }

    /// The writer. What is committed under it is said when the outermost hold is let go of, as
    /// [`Topics::ALL`] unless a write said what it touches through [`Self::touch`].
    pub(crate) fn connection(&self) -> Writer<'_> {
        let held = self.db.lock();
        self.changes.enter(held.total_changes());
        Writer {
            held,
            changes: &self.changes,
        }
    }

    /// The writer, for a write that touches `topics`.
    pub(crate) fn touch(&self, topics: Topics) -> Writer<'_> {
        let writer = self.connection();
        self.changes.declare(topics);
        writer
    }

    /// Hear every commit this store makes from now on. Not another process's, which are heard by
    /// watching [`Self::file`] and asking [`Self::data_version`].
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<Committed> {
        self.changes.subscribe()
    }

    /// How many connections read beside the writer: none in memory.
    pub fn reader_count(&self) -> usize {
        self.readers.len()
    }

    /// The generation of the last commit said. Read before a read begins, it is a commit that
    /// read is sure to see.
    pub fn generation(&self) -> Generation {
        self.changes.generation()
    }

    /// Run `read` with every read on this thread seeing one moment of the database. Nested, or
    /// inside a write, it simply runs.
    pub fn read_pinned<T>(&self, read: impl FnOnce() -> T) -> Result<T, StoreError> {
        let held_already = self.db.is_owned_by_current_thread()
            || self
                .readers
                .iter()
                .any(ReentrantMutex::is_owned_by_current_thread);
        if held_already {
            return Ok(read());
        }
        if self.readers.is_empty() {
            let _writer = self.db.lock();
            return Ok(read());
        }
        let pinned = Pinned::begin(self.reader())?;
        let answer = read();
        drop(pinned);
        Ok(answer)
    }

    /// The database's file, when it is in one.
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// A number that changes once another connection (another process) has committed. `None`
    /// while the writer is busy on another thread; asked again, it answers.
    pub fn data_version(&self) -> Option<i64> {
        self.db
            .try_lock()?
            .query_row("PRAGMA data_version", [], |row| row.get(0))
            .ok()
    }

    /// A connection to read with: the writer again for a thread that holds it (so a read inside
    /// a write reads that write), the thread's pinned reader, else the first free reader.
    pub(crate) fn reader(&self) -> ReentrantMutexGuard<'_, Connection> {
        if self.db.is_owned_by_current_thread() {
            return self.db.lock();
        }
        if let Some(pinned) = self
            .readers
            .iter()
            .find(|reader| reader.is_owned_by_current_thread())
        {
            return pinned.lock();
        }
        for reader in &self.readers {
            if let Some(free) = reader.try_lock() {
                return free;
            }
        }
        match self.readers.first() {
            Some(reader) => reader.lock(),
            None => self.db.lock(),
        }
    }
}

/// The writer, held: a connection by `Deref`, which says what was committed under it when the
/// outermost hold is let go of, while the lock is still held, so commits are told in order.
pub struct Writer<'a> {
    held: ReentrantMutexGuard<'a, Connection>,
    changes: &'a Changes,
}

impl std::ops::Deref for Writer<'_> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.held
    }
}

impl Drop for Writer<'_> {
    fn drop(&mut self) {
        self.changes.leave(self.held.total_changes());
    }
}

impl std::fmt::Debug for Writer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Writer")
    }
}

/// A reader held for one thread in one read transaction, for [`SqliteStore::read_pinned`].
struct Pinned<'a>(ReentrantMutexGuard<'a, Connection>);

impl<'a> Pinned<'a> {
    fn begin(reader: ReentrantMutexGuard<'a, Connection>) -> Result<Self, StoreError> {
        reader.execute_batch("BEGIN")?;
        let pinned = Self(reader);
        // A deferred transaction takes its snapshot at its first read: this is that read.
        pinned
            .0
            .query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))?;
        Ok(pinned)
    }
}

impl Drop for Pinned<'_> {
    fn drop(&mut self) {
        // A read transaction has nothing to keep; SQLite ends it either way.
        let _ = self.0.execute_batch("COMMIT");
    }
}
