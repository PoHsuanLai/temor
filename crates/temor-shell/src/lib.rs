//! temor's shell: which shell the person uses, the environment a child starts with, and the
//! shell integration (OSC 133 prompt marks and OSC 7 working directory) laid down for it.
//!
//! This crate is pure planning plus the shim files it writes under a caller's runtime directory.
//! It reads no ambient state: the environment, the passwd entry and the runtime directory are
//! handed in. It spawns nothing; `temor-pty` starts the shell this crate plans.
//!
//! # Example
//!
//! The shell is the first usable of the passwd entry, then `SHELL` from the environment:
//!
//! ```
//! use temor_shell::{ChildEnv, ShellKind, resolve_shell};
//!
//! let env: ChildEnv = [("SHELL", "/usr/bin/fish")].into_iter().collect();
//! let shell = resolve_shell(None, &env, |_| true);
//! assert_eq!(shell.kind, ShellKind::Fish);
//! ```

mod env;
mod error;
mod osc7;
mod shell;

pub use env::ChildEnv;
pub use error::ShellError;
pub use osc7::{cwd_from_url, percent_decode_strict};
pub use shell::{
    IntegrationPlan, IntegrationState, Shell, ShellKind, ShellScript, ShellStart, ShimFile, UserId,
    integration_scripts, passwd_login_shell, plan_integration, resolve_shell, write_shims,
};
