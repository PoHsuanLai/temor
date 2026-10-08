//! Linux: the pseudo-terminal through rustix (no `unsafe`), and process facts from `/proc`.

mod os;
mod pty;

pub(crate) use os::Linux;
pub use pty::{Pty, PtyIo, run_session_leader};
