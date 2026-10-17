//! The real user id of this process, read through the OS edge.

use std::path::PathBuf;

use temor_shell::UserId;

use crate::platform::{Current, Os};

/// The real user id of this process, where the OS has one temor can read.
pub fn current_user() -> Option<UserId> {
    <Current as Os>::current_user()
}

/// This person's login shell, from the system's account lookup (on Linux through NSS, so an
/// account from LDAP, sssd or systemd-homed has one too). `None` when there is none to read.
pub fn login_shell() -> Option<PathBuf> {
    <Current as Os>::login_shell(current_user()?)
}
