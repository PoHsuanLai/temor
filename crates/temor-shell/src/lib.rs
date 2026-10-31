//! temor's shell: which shell the person uses, the environment a child starts with, and the
//! shell integration (OSC 133 prompt marks and OSC 7 working directory) laid down for it, and the
//! formats its scripts answer in ([`report`], [`complete`]), and the vocabulary of the sources that complete
//! the composer's line ([`suggest`]).
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

pub mod complete;
mod env;
mod error;
mod osc7;
mod platform;
mod quote;
mod replay;
pub mod report;
mod shell;
pub mod suggest;

pub use env::ChildEnv;
pub use error::ShellError;
pub use osc7::{cwd_from_url, percent_decode_strict};
pub use platform::SOCKET_PATH_MAX;
#[cfg(feature = "io")]
pub use platform::{ensure_private_dir, ensure_private_dir_all};
pub use quote::sh_quote;
pub use replay::alias_script;
#[cfg(feature = "io")]
pub use shell::write_shims;
pub use shell::{
    BashAdapter, FishAdapter, IntegrationPlan, NuAdapter, PROBE_WORD, PlainAdapter, Shell,
    ShellAdapter, ShellKind, ShellScript, ShellStart, ShimFile, UserId, XDG_DATA_DIRS_DEFAULT,
    ZshAdapter, adapter_for, integration_scripts, plan_integration, probe_line, resolve_shell,
};
