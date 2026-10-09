//! temor's shell: which shell the person uses, the environment a child starts with, and the
//! shell integration (OSC 133 prompt marks and OSC 7 working directory) laid down for it.
//!
//! This crate is pure planning plus the shim files it writes under a caller's runtime directory.
//! It reads no ambient state: the environment, the passwd entry and the runtime directory are
//! handed in. It spawns nothing; `temor-pty` starts the shell this crate plans.

mod env;
mod error;
mod shell;

pub use env::ChildEnv;
pub use error::ShellError;
pub use shell::{
    IntegrationPlan, IntegrationState, Shell, ShellKind, ShellScript, ShellStart, ShimFile, UserId,
    integration_scripts, passwd_login_shell, plan_integration, resolve_shell, write_shims,
};
