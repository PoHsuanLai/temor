//! A Space's secrets in any porter store, under temor's own name, so no other program's items are
//! read or written. No porter daemon is involved. A secret is handed to porter's `SecretText`,
//! which wipes itself, and is never formatted or logged. The live stores are each platform's
//! `SystemSecrets` (the Secret Service on Linux, the login Keychain on macOS).

use crate::secrets::{SecretFault, SecretVault};
use porter_core::{AccountId, Credential, SecretKey, SecretPurpose, SecretText};
use porter_secrets::{Secrets, SecretsError};
use std::fmt;
use tokio::runtime::Handle;

/// The name temor's items are filed under in the store.
pub(crate) const NAMESPACE: &str = "temor";

/// Secrets in any porter store; the live one is [`SystemSecrets`]. Calls block the thread they
/// are made on until `runtime` has the answer, so they belong on a worker thread.
#[derive(Debug)]
pub(crate) struct StoredSecrets<S> {
    store: S,
    runtime: Handle,
}

impl<S: Secrets> StoredSecrets<S> {
    pub(crate) fn over(store: S, runtime: Handle) -> Self {
        Self { store, runtime }
    }
}

fn key_of(account: &str) -> Result<SecretKey, SecretFault> {
    let account = AccountId::parse(account).map_err(|_| SecretFault::BadName)?;
    Ok(SecretKey {
        account,
        purpose: SecretPurpose::ApiKey,
    })
}

fn fault_of(error: SecretsError) -> SecretFault {
    match error {
        SecretsError::Locked => SecretFault::Locked,
        SecretsError::Unreadable => SecretFault::Unreadable,
        // `Missing` is answered before it gets here; the rest means no store answered.
        _ => SecretFault::NoStore,
    }
}

impl<S: Secrets> SecretVault for StoredSecrets<S>
where
    S: fmt::Debug,
{
    fn get(&self, account: &str) -> Result<Option<String>, SecretFault> {
        let key = key_of(account)?;
        match self.runtime.block_on(self.store.get(&key)) {
            Ok(Credential::ApiKey(text)) => Ok(Some(text.expose().to_owned())),
            Ok(_) => Err(SecretFault::Unreadable),
            Err(SecretsError::Missing) => Ok(None),
            Err(other) => Err(fault_of(other)),
        }
    }

    fn put(&self, account: &str, secret: &str) -> Result<(), SecretFault> {
        let key = key_of(account)?;
        let value = Credential::ApiKey(SecretText::new(secret));
        self.runtime
            .block_on(self.store.put(&key, &value))
            .map_err(fault_of)
    }

    fn delete(&self, account: &str) -> Result<(), SecretFault> {
        let key = key_of(account)?;
        self.runtime
            .block_on(self.store.delete(&key))
            .map_err(fault_of)
    }
}
