//! The exact `ssh` argv for a shared connection per host (PLAN §9b): the session through the
//! master, its control commands (`-O check`, `-O exit`) and port forwards (`-O forward`).

mod build;
mod control;
mod forward;
#[cfg(test)]
mod tests;

pub use build::{
    RemoteCommand, cancel_forward_argv, check_argv, exit_argv, forward_argv, plain_session_argv,
    session_argv,
};
pub use control::{ControlLayout, ControlPersist, RuntimeDir, Seconds};
pub use forward::{BindAddress, Forward, ForwardSpec, RemoteHost};
