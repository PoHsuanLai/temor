//! Where a Space's secrets are kept (PLAN §7): the person's password store. The window asks
//! through [`SecretVault`]; the Linux build keeps them in the Secret Service through porter's
//! store ([`crate::SystemSecrets`]), and every other build has [`NoSecrets`]. A secret goes from the
//! caller to the store and back, and nowhere else: it is not formatted into text, logged or
//! written to a file here, and a fault names no value.

use std::fmt;

/// Why a secret could not be kept or read; the text is for the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SecretFault {
    /// This computer has no password store to use (or this build has none).
    #[error("There is nowhere to keep secrets on this computer yet.")]
    NoStore,
    /// The store is locked and was not unlocked.
    #[error("The password store is locked. Unlock it and try again.")]
    Locked,
    /// Something is kept under that name, but it is not a saved value temor can read.
    #[error("The saved value could not be read.")]
    Unreadable,
    /// The name is not one the store can file a secret under.
    #[error("That name cannot be used to keep a secret.")]
    BadName,
}

/// A store of secrets filed by account name. Calls wait for the store, so they belong on a
/// worker thread or a thread that is not inside an async task.
pub trait SecretVault: Send + Sync + fmt::Debug {
    /// The secret kept for `account`, `None` when there is none.
    fn get(&self, account: &str) -> Result<Option<String>, SecretFault>;

    /// Keeps `secret` for `account`, replacing one that is there.
    fn put(&self, account: &str, secret: &str) -> Result<(), SecretFault>;

    /// Removes the secret of `account`; none there is not a fault.
    fn delete(&self, account: &str) -> Result<(), SecretFault>;
}

/// A store with nothing in it: every ask finds nothing and every keep is refused.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSecrets;

impl SecretVault for NoSecrets {
    fn get(&self, _account: &str) -> Result<Option<String>, SecretFault> {
        Ok(None)
    }

    fn put(&self, _account: &str, _secret: &str) -> Result<(), SecretFault> {
        Err(SecretFault::NoStore)
    }

    fn delete(&self, _account: &str) -> Result<(), SecretFault> {
        Err(SecretFault::NoStore)
    }
}
