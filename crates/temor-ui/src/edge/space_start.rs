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

/// Where a Space's secrets come from: the account store. `Send + Sync` so it can be handed to
/// the window as a root context.
pub trait CredentialSource: Send + Sync {
    /// The secret of `account`, `None` when the store has no such account.
    fn secret(&self, account: &AccountRef) -> Result<Option<String>, CredentialFault>;
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
