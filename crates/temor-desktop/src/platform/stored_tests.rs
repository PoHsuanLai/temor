use crate::platform::stored::StoredSecrets;
use crate::secrets::{SecretFault, SecretVault};
use porter_core::{AccountId, Credential, SecretKey, SecretPurpose, SecretText};
use porter_secrets::{MemorySecrets, Secrets};
use tokio::runtime::{Builder, Runtime};

fn runtime() -> Runtime {
    Builder::new_current_thread().build().unwrap()
}

const ACCOUNT: &str = "temor.space1.api_key";

#[test]
fn a_secret_is_kept_replaced_read_and_removed() {
    let rt = runtime();
    let vault = StoredSecrets::over(MemorySecrets::default(), rt.handle().clone());
    assert_eq!(vault.get(ACCOUNT), Ok(None));
    vault.put(ACCOUNT, "first").unwrap();
    assert_eq!(vault.get(ACCOUNT), Ok(Some("first".to_owned())));
    vault.put(ACCOUNT, "second").unwrap();
    assert_eq!(vault.get(ACCOUNT), Ok(Some("second".to_owned())));
    // A second account is its own.
    assert_eq!(vault.get("temor.space2.api_key"), Ok(None));
    vault.delete(ACCOUNT).unwrap();
    assert_eq!(vault.get(ACCOUNT), Ok(None));
    // Removing what is not there is not a fault.
    vault.delete(ACCOUNT).unwrap();
}

#[test]
fn a_name_the_store_cannot_file_is_refused_before_it_is_asked() {
    let rt = runtime();
    let vault = StoredSecrets::over(MemorySecrets::default(), rt.handle().clone());
    for name in ["", "Has Capitals", "a/b"] {
        assert_eq!(vault.get(name), Err(SecretFault::BadName), "{name:?}");
        assert_eq!(vault.put(name, "x"), Err(SecretFault::BadName), "{name:?}");
        assert_eq!(vault.delete(name), Err(SecretFault::BadName), "{name:?}");
    }
}

#[test]
fn an_item_that_is_not_an_api_key_reads_as_unreadable() {
    let rt = runtime();
    let store = MemorySecrets::default();
    let key = SecretKey {
        account: AccountId::parse(ACCOUNT).unwrap(),
        purpose: SecretPurpose::ApiKey,
    };
    rt.block_on(store.put(&key, &Credential::Password(SecretText::new("p"))))
        .unwrap();
    let vault = StoredSecrets::over(store, rt.handle().clone());
    assert_eq!(vault.get(ACCOUNT), Err(SecretFault::Unreadable));
}
