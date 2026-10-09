//! What `temor view` does. The viewer is on the person's computer, so over SSH the file goes to
//! the shelf instead (the shelf row opens it). Pure.

use std::path::PathBuf;

/// Whether this command runs on the person's own computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    Here,
    OverSsh,
}

/// What to do with the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewPlan {
    /// Open it with the system's program.
    OpenHere(PathBuf),
    /// Send it to the shelf; the person opens it there.
    SendToShelf(PathBuf),
}

pub fn plan(file: PathBuf, at: Where) -> ViewPlan {
    match at {
        Where::Here => ViewPlan::OpenHere(file),
        Where::OverSsh => ViewPlan::SendToShelf(file),
    }
}

/// Reads the environment SSH sets for its sessions.
pub fn where_from(
    ssh_connection: Option<&std::ffi::OsStr>,
    ssh_tty: Option<&std::ffi::OsStr>,
) -> Where {
    match (ssh_connection, ssh_tty) {
        (None, None) => Where::Here,
        _ => Where::OverSsh,
    }
}
