//! Forward-only schema migrations.

use rusqlite::Connection;

use crate::StoreError;
use crate::error::Context;

/// Every migration, in order. Index 0 makes schema version 1.
///
/// **Append only.** A released migration is a fact about databases already on disk; editing one
/// does not change them, it only makes this build disagree with them.
const MIGRATIONS: &[(u32, &str)] = &[(1, include_str!("../migrations/0001_initial.sql"))];

/// The schema version this build expects.
pub const EXPECTED_VERSION: u32 = 1;

/// Bring `db` up to [`EXPECTED_VERSION`], making it if it is empty. Refuses a database a newer
/// build wrote.
pub(crate) fn migrate(db: &Connection) -> Result<(), StoreError> {
    db.execute_batch("PRAGMA foreign_keys = ON;")?;
    let current = current_version(db)?;
    if current > EXPECTED_VERSION {
        return Err(StoreError::SchemaTooNew {
            found: current,
            expected: EXPECTED_VERSION,
        });
    }
    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        // One transaction per migration: a failure half way leaves the database at the last
        // version that fully applied, not in a shape no version describes.
        let tx = db.unchecked_transaction()?;
        tx.execute_batch(sql).map_err(|e| {
            StoreError::db(Context {
                doing: format!("migration {version}"),
                source: e.into(),
            })
        })?;
        // 0001 writes its own row; later migrations are recorded here.
        if *version > 1 {
            tx.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
                [version],
            )?;
        }
        tx.commit()?;
    }
    Ok(())
}

/// The highest version applied, or 0 for an empty database.
fn current_version(db: &Connection) -> Result<u32, StoreError> {
    let exists: bool = db.query_row(
        "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(0);
    }
    Ok(db.query_row(
        "SELECT coalesce(max(version), 0) FROM schema_version",
        [],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_database_is_refused_rather_than_downgraded() {
        let db = Connection::open_in_memory().expect("open");
        migrate(&db).expect("migrate");
        db.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (99, datetime('now'))",
            [],
        )
        .expect("insert");
        match migrate(&db) {
            Err(StoreError::SchemaTooNew { found, expected }) => {
                assert_eq!((found, expected), (99, EXPECTED_VERSION));
            }
            other => panic!("expected SchemaTooNew, got {other:?}"),
        }
    }

    #[test]
    fn migrating_twice_changes_nothing() {
        let db = Connection::open_in_memory().expect("open");
        migrate(&db).expect("first");
        migrate(&db).expect("second");
        assert_eq!(current_version(&db).expect("version"), EXPECTED_VERSION);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        // SQLite ignores the declaration unless the pragma is on for the connection; without it a
        // session's commands would outlive the session.
        let db = Connection::open_in_memory().expect("open");
        migrate(&db).expect("migrate");
        let on: bool = db
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("pragma");
        assert!(on);
    }
}
