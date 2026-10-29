//! A Space's credentials (PLAN §7): the profile keeps only which account fills which variable;
//! the secret lives in the account store (`CredentialSource`), where it is set, replaced and
//! removed from here. Nothing in this file keeps a secret, formats one into text or returns one:
//! it goes from the field to the store and no further, and an error names the variable, never the
//! value.

use ds::components::app::spaces::SpaceId;
use temor_client::model::space::{AccountRef, Credential};
use temor_client::model::vocab::text::EnvKey;

use super::space_edit::{account_for, credentials_with, credentials_without, variable_name};
use crate::edge::CredentialSource;

/// Whether the store holds the secret of `account`. A store that cannot answer reads as not set.
pub fn held(store: &dyn CredentialSource, account: &AccountRef) -> bool {
    store.holds(account).unwrap_or(false)
}

/// Replaces the secret of `account` with `secret`. An empty secret is refused.
pub fn replace(
    store: &dyn CredentialSource,
    account: &AccountRef,
    secret: &str,
) -> Result<(), String> {
    if secret.is_empty() {
        return Err("Type the new value first.".to_owned());
    }
    store
        .set_secret(account, secret)
        .map_err(|why| why.to_string())
}

/// `list` with a credential for the variable `name` in Space `space`, its secret kept in the
/// store under an account named for both.
pub fn add(
    store: &dyn CredentialSource,
    space: SpaceId,
    list: Vec<Credential>,
    name: &str,
    secret: &str,
) -> Result<Vec<Credential>, String> {
    let env = variable_name(name)?;
    let account = account_for(space, &env)?;
    replace(store, &account, secret)?;
    Ok(credentials_with(list, env, account))
}

/// `list` without the credential for `env`, its secret taken out of the store first: a store that
/// refuses leaves the credential where it is.
pub fn remove(
    store: &dyn CredentialSource,
    list: Vec<Credential>,
    env: &EnvKey,
) -> Result<Vec<Credential>, String> {
    if let Some(have) = list.iter().find(|have| have.env == *env) {
        store
            .remove_secret(&have.account)
            .map_err(|why| why.to_string())?;
    }
    Ok(credentials_without(list, env))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use super::*;
    use crate::edge::{CredentialFault, NoCredentials};

    /// A store in memory.
    #[derive(Default)]
    struct Memory(Mutex<BTreeMap<String, String>>);

    impl CredentialSource for Memory {
        fn secret(&self, account: &AccountRef) -> Result<Option<String>, CredentialFault> {
            Ok(self.0.lock().expect("lock").get(account.as_str()).cloned())
        }
        fn set_secret(&self, account: &AccountRef, secret: &str) -> Result<(), CredentialFault> {
            self.0
                .lock()
                .expect("lock")
                .insert(account.as_str().to_owned(), secret.to_owned());
            Ok(())
        }
        fn remove_secret(&self, account: &AccountRef) -> Result<(), CredentialFault> {
            self.0.lock().expect("lock").remove(account.as_str());
            Ok(())
        }
    }

    #[test]
    fn a_secret_is_set_replaced_and_removed_and_the_profile_keeps_only_the_account() {
        let store = Memory::default();
        let list = add(&store, SpaceId(1), Vec::new(), "API_KEY", "s3cret").expect("added");
        assert_eq!(list.len(), 1);
        let account = list[0].account.clone();
        assert!(held(&store, &account));
        assert_eq!(
            store.secret(&account).expect("ok").as_deref(),
            Some("s3cret")
        );
        // The credential as the file would hold it has no value in it.
        assert!(!format!("{list:?}").contains("s3cret"));

        replace(&store, &account, "other").expect("replaced");
        assert_eq!(
            store.secret(&account).expect("ok").as_deref(),
            Some("other")
        );
        assert!(replace(&store, &account, "").is_err());

        let none = remove(&store, list, &EnvKey::new("API_KEY").expect("a name")).expect("removed");
        assert!(none.is_empty());
        assert!(!held(&store, &account));
    }

    #[test]
    fn a_store_that_cannot_keep_secrets_refuses_and_the_profile_is_unchanged() {
        let refused = add(&NoCredentials, SpaceId(1), Vec::new(), "API_KEY", "s3cret");
        let why = refused.expect_err("nowhere to keep it");
        assert!(!why.contains("s3cret"), "{why}");
        let key = EnvKey::new("API_KEY").expect("a name");
        let account = account_for(SpaceId(1), &key).expect("account");
        let list = vec![Credential {
            env: key.clone(),
            account,
        }];
        assert!(remove(&NoCredentials, list, &key).is_err());
    }
}
