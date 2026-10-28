//! A Space's secrets from the desktop's password store: [`CredentialSource`] over the
//! `SecretVault` temor-desktop provides. A secret passes through here to the store or back and is
//! never formatted into text, logged or kept; a fault is the store's own words.

use std::sync::Arc;

use temor_desktop::{SecretFault, SecretVault};
use temor_layout::space::AccountRef;

use super::start::{CredentialFault, CredentialSource, Credentials};

/// The credentials of a Space, kept in `vault`.
#[derive(Debug, Clone)]
pub struct VaultCredentials(pub Arc<dyn SecretVault>);

impl VaultCredentials {
    /// `vault` as the window's [`Credentials`].
    pub fn credentials(vault: Arc<dyn SecretVault>) -> Credentials {
        Credentials(Arc::new(VaultCredentials(vault)))
    }
}

fn fault(why: SecretFault) -> CredentialFault {
    CredentialFault(why.to_string())
}

impl CredentialSource for VaultCredentials {
    fn secret(&self, account: &AccountRef) -> Result<Option<String>, CredentialFault> {
        self.0.get(account.as_str()).map_err(fault)
    }

    fn set_secret(&self, account: &AccountRef, secret: &str) -> Result<(), CredentialFault> {
        self.0.put(account.as_str(), secret).map_err(fault)
    }

    fn remove_secret(&self, account: &AccountRef) -> Result<(), CredentialFault> {
        self.0.delete(account.as_str()).map_err(fault)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use temor_desktop::NoSecrets;

    use super::*;
    use crate::space::start::Held;

    /// A vault in memory; its `Debug` shows no secret.
    #[derive(Default)]
    struct Memory(Mutex<HashMap<String, String>>);

    impl std::fmt::Debug for Memory {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Memory")
        }
    }

    impl SecretVault for Memory {
        fn get(&self, account: &str) -> Result<Option<String>, SecretFault> {
            Ok(self.0.lock().unwrap().get(account).cloned())
        }

        fn put(&self, account: &str, secret: &str) -> Result<(), SecretFault> {
            self.0
                .lock()
                .unwrap()
                .insert(account.to_owned(), secret.to_owned());
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<(), SecretFault> {
            self.0.lock().unwrap().remove(account);
            Ok(())
        }
    }

    fn account() -> AccountRef {
        AccountRef::new("temor.space1.api_key").unwrap()
    }

    #[test]
    fn a_secret_set_is_held_read_and_gone_once_removed() {
        let source = VaultCredentials(Arc::new(Memory::default()));
        assert_eq!(source.holds(&account()), Ok(Held::NotSet));
        source.set_secret(&account(), "s3cret").unwrap();
        assert_eq!(source.holds(&account()), Ok(Held::Set));
        assert_eq!(source.secret(&account()), Ok(Some("s3cret".to_owned())));
        source.remove_secret(&account()).unwrap();
        assert_eq!(source.secret(&account()), Ok(None));
    }

    #[test]
    fn without_a_store_nothing_is_held_and_keeping_says_why() {
        let source = VaultCredentials(Arc::new(NoSecrets));
        assert_eq!(source.holds(&account()), Ok(Held::NotSet));
        let refused = source.set_secret(&account(), "s3cret").unwrap_err();
        assert_eq!(
            refused.to_string(),
            "There is nowhere to keep secrets on this computer yet."
        );
        assert!(source.remove_secret(&account()).is_err());
    }
}
