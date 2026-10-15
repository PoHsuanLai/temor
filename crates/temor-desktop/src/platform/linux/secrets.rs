//! A Space's secrets in the Secret Service, through porter's store under temor's own name, so
//! no other program's items are read or written. No porter daemon is involved; where there is
//! no Secret Service (a bare session) every ask is refused in the usual words. A secret is
//! handed to porter's `SecretText`, which wipes itself, and is never formatted or logged.

use crate::secrets::{SecretFault, SecretVault};
use porter_core::{AccountId, Credential, SecretKey, SecretPurpose, SecretText};
use porter_secrets::{Namespace, Oo7NamedSecrets, Oo7Secrets, Secrets, SecretsError};
use std::fmt;
use tokio::runtime::Handle;

/// The name temor's items are filed under in the store.
const NAMESPACE: &str = "temor";

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

/// The person's Secret Service, under temor's name.
#[derive(Debug)]
pub struct SystemSecrets {
    inner: StoredSecrets<Oo7NamedSecrets>,
}

impl SystemSecrets {
    /// The store, with calls waiting on `runtime`, which the caller keeps alive. The Secret
    /// Service is first opened by the first ask, not here.
    pub fn new(runtime: Handle) -> Result<Self, SecretFault> {
        let namespace = Namespace::parse(NAMESPACE).map_err(|_| SecretFault::NoStore)?;
        Ok(Self {
            inner: StoredSecrets::over(Oo7Secrets::in_namespace(namespace), runtime),
        })
    }
}

impl SecretVault for SystemSecrets {
    fn get(&self, account: &str) -> Result<Option<String>, SecretFault> {
        self.inner.get(account)
    }

    fn put(&self, account: &str, secret: &str) -> Result<(), SecretFault> {
        self.inner.put(account, secret)
    }

    fn delete(&self, account: &str) -> Result<(), SecretFault> {
        self.inner.delete(account)
    }
}
