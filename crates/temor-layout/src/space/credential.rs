//! Credentials in a profile are references to porter's account store, never values (PLAN §7):
//! the profile says which account fills which variable, and the edge asks the store for the
//! secret as a shell starts.

use serde::{Deserialize, Serialize};
use temor_core::error::CoreError;
use temor_core::vocab::text::EnvKey;

/// The longest account id, in bytes (porter's limit).
const MOST_BYTES: usize = 64;

/// The id of one account in porter's store: a slug of lowercase letters and digits, then
/// letters, digits, `.`, `_` or `-`, at most 64 bytes. Checked where it is read, so a file
/// cannot make the edge ask the store for something that is not an account.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AccountRef(String);

impl AccountRef {
    /// `text` as an account reference, or why it cannot be one.
    pub fn new(text: impl Into<String>) -> Result<AccountRef, CoreError> {
        let text = text.into();
        let mut bytes = text.bytes();
        let first = bytes
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        let rest = bytes.all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
        if first && rest && text.len() <= MOST_BYTES {
            Ok(AccountRef(text))
        } else {
            Err(CoreError::BadAccountRef(text))
        }
    }

    /// The id's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AccountRef {
    type Error = CoreError;
    fn try_from(text: String) -> Result<AccountRef, CoreError> {
        AccountRef::new(text)
    }
}

impl From<AccountRef> for String {
    fn from(account: AccountRef) -> String {
        account.0
    }
}

/// One variable filled from an account: the shell gets `env` set to that account's secret.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Credential {
    /// The variable the secret goes in.
    pub env: EnvKey,
    /// The account that holds it.
    pub account: AccountRef,
}
