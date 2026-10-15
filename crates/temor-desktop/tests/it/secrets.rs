//! A Space's secret in a real Secret Service, through the same `SystemSecrets` the window uses.
//! It must never reach the person's own password store, so it is ignored and also wants
//! `TEMOR_SECRETS_TEST=1`. Run it by hand inside a private bus with a scratch store, e.g.
//! `dbus-run-session -- /mnt/data/cargo/hw-tools/bin/secret-service -- env TEMOR_SECRETS_TEST=1
//! cargo test -p temor-desktop --test it secrets -- --ignored` (a gnome-keyring with its own
//! data folder, unlocked with a test password).

#![cfg(all(target_os = "linux", feature = "quire-desktop"))]

use temor_desktop::{SecretVault, SystemSecrets};

#[test]
#[ignore = "needs a scratch Secret Service on a private bus: see the module notes"]
fn a_space_secret_is_kept_read_back_replaced_and_removed() {
    assert_eq!(
        std::env::var("TEMOR_SECRETS_TEST").as_deref(),
        Ok("1"),
        "refusing to touch a password store that may be the person's own"
    );
    // As the program has it: calls block on a handle, which only a multi-thread runtime drives.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let vault = SystemSecrets::new(runtime.handle().clone()).expect("store");
    let account = "temor.space7.staging";
    assert_eq!(vault.get(account), Ok(None));
    vault.put(account, "first token").expect("kept");
    assert_eq!(vault.get(account), Ok(Some("first token".to_owned())));
    vault.put(account, "second token").expect("replaced");
    assert_eq!(vault.get(account), Ok(Some("second token".to_owned())));
    assert_eq!(vault.get("temor.space7.prod"), Ok(None), "another account");
    vault.delete(account).expect("removed");
    assert_eq!(vault.get(account), Ok(None));
    vault
        .delete(account)
        .expect("removing nothing is not a fault");
}
