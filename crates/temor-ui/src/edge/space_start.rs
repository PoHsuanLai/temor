//! What a Space's new shell is given from outside the process (PLAN §7): the dotenv file under
//! the project root, and the secrets from the account store. The store belongs to the desktop
//! integration (temor-desktop, through porter); the window only asks through
//! [`CredentialSource`], whose default has nothing.

use std::fmt;
use std::io::Read;
use std::sync::Arc;

use temor_core::space::AccountRef;

/// The most of a dotenv file that is read; a larger file is not a dotenv file.
const DOTENV_LIMIT: u64 = 64 * 1024;

/// The text of the dotenv file at `path`, when it can be read as text within the limit.
pub fn read_dotenv(path: &str) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(DOTENV_LIMIT + 1).read_to_string(&mut text).ok()?;
    (text.len() as u64 <= DOTENV_LIMIT).then_some(text)
}

/// Why a secret could not be had; the text is for the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialFault(pub String);

impl fmt::Display for CredentialFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CredentialFault {}

/// Whether the store holds a secret for an account. Asked instead of the secret itself wherever
/// only "is it set" is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Set,
    NotSet,
}

/// Where a Space's secrets come from: the account store. `Send + Sync` so it can be handed to
/// the window as a root context.
pub trait CredentialSource: Send + Sync {
    /// The secret of `account`, `None` when the store has no such account.
    fn secret(&self, account: &AccountRef) -> Result<Option<String>, CredentialFault>;

    /// Whether the store has a secret for `account`.
    fn holds(&self, account: &AccountRef) -> Result<Held, CredentialFault> {
        Ok(match self.secret(account)? {
            Some(_) => Held::Set,
            None => Held::NotSet,
        })
    }

    /// Keeps `secret` as the secret of `account`, replacing one that is there. The Settings
    /// window is the only caller; the secret goes nowhere else (not to a log, not to the file).
    fn set_secret(&self, _account: &AccountRef, _secret: &str) -> Result<(), CredentialFault> {
        Err(no_store())
    }

    /// Takes the secret of `account` out of the store; none there is not an error.
    fn remove_secret(&self, _account: &AccountRef) -> Result<(), CredentialFault> {
        Err(no_store())
    }
}

/// The fault a store that is not there gives, in the words temor-desktop uses for it.
fn no_store() -> CredentialFault {
    CredentialFault(temor_desktop::SecretFault::NoStore.to_string())
}

/// A store with nothing in it: the default until the desktop integration provides one.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCredentials;

impl CredentialSource for NoCredentials {
    fn secret(&self, _: &AccountRef) -> Result<Option<String>, CredentialFault> {
        Ok(None)
    }
}

/// The source the window holds. Give one to the window with `with_context` on its launch
/// config; without one the window uses [`NoCredentials`].
#[derive(Clone)]
pub struct Credentials(pub Arc<dyn CredentialSource>);

impl Default for Credentials {
    fn default() -> Self {
        Credentials(Arc::new(NoCredentials))
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credentials")
    }
}
