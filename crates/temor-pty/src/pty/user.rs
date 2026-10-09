//! The real user id of this process, read through the OS edge.

use temor_shell::UserId;

use crate::platform::{Current, Os};

/// The real user id of this process, where the OS has one temor can read.
pub fn current_user() -> Option<UserId> {
    <Current as Os>::current_user()
}
