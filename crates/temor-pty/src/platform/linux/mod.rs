//! Linux: the pseudo-terminal through rustix (no `unsafe`), and process facts from `/proc`.

mod os;
#[cfg(test)]
mod os_tests;
mod pty;

pub(crate) use os::Linux;
pub use pty::{Pty, PtyIo, run_session_leader};
