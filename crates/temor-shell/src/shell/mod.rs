//! The person's shell and temor's shell integration for it.

mod adapter;
mod io;
mod plan;
mod probe;
mod resolve;
mod scripts;
#[cfg(test)]
mod tests;

pub use adapter::{
    BashAdapter, FishAdapter, NuAdapter, PlainAdapter, ShellAdapter, ZshAdapter, adapter_for,
};
pub use io::write_shims;
pub use plan::{
    IntegrationPlan, IntegrationState, ShellStart, ShimFile, XDG_DATA_DIRS_DEFAULT,
    plan_integration,
};
pub use probe::{PROBE_WORD, probe_line};
pub use resolve::{Shell, ShellKind, UserId, passwd_login_shell, resolve_shell};
pub use scripts::{ShellScript, integration_scripts};
