//! A Space's secrets in the Secret Service, through porter's store under temor's own name, so
//! no other program's items are read or written. No porter daemon is involved; where there is
//! no Secret Service (a bare session) every ask is refused in the usual words.

use crate::platform::stored::{NAMESPACE, StoredSecrets};
use crate::secrets::{SecretFault, SecretVault};
use porter_secrets::{Namespace, Oo7NamedSecrets, Oo7Secrets};
use tokio::runtime::Handle;

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
