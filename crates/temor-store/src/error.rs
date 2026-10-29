//! Store failures.

/// What a failure carries as its cause, kept as the error it was rather than as its text.
pub type Source = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Something went wrong where the data is kept.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database refused or could not be reached. The cause is kept, so a caller that wants
    /// to look can `source()` its way to it.
    #[error("database: {0}")]
    Db(#[source] Source),
    /// A stored value no longer reads as its type: almost always a migration that is missing.
    #[error("cannot read stored {what}: {why}")]
    Decode { what: &'static str, why: String },
    /// Written by a newer temor. Never migrated backwards: guessing what a later version meant
    /// and guessing wrong would lose the person's history without a word.
    #[error("the database is newer than this build: schema v{found}, expected v{expected}")]
    SchemaTooNew { found: u32, expected: u32 },
}

impl StoreError {
    /// A failure of the database, whatever it was.
    pub(crate) fn db(cause: impl Into<Source>) -> Self {
        StoreError::Db(cause.into())
    }

    /// Whether the database said "not now" rather than "no": another connection holds the lock
    /// (`SQLITE_BUSY`, `SQLITE_LOCKED`), so going away and trying again is the remedy.
    pub fn is_busy(&self) -> bool {
        let StoreError::Db(cause) = self else {
            return false;
        };
        matches!(
            cause
                .downcast_ref::<rusqlite::Error>()
                .and_then(rusqlite::Error::sqlite_error_code),
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
        )
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(cause: rusqlite::Error) -> Self {
        StoreError::db(cause)
    }
}

/// Words about what was being done, kept in front of the error that stopped it.
#[derive(Debug, thiserror::Error)]
#[error("{doing}: {source}")]
pub(crate) struct Context {
    pub doing: String,
    #[source]
    pub source: Source,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(code: i32) -> StoreError {
        StoreError::db(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(code),
            None,
        ))
    }

    #[test]
    fn a_contended_lock_is_busy_and_nothing_else_is() {
        assert!(failure(rusqlite::ffi::SQLITE_BUSY).is_busy());
        assert!(failure(rusqlite::ffi::SQLITE_LOCKED).is_busy());
        assert!(!failure(rusqlite::ffi::SQLITE_CORRUPT).is_busy());
        assert!(
            !StoreError::SchemaTooNew {
                found: 2,
                expected: 1
            }
            .is_busy()
        );
    }
}
