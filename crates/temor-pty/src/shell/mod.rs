//! The person's shell and temor's shell integration for it.

mod io;
mod plan;
mod resolve;
mod scripts;
#[cfg(test)]
mod tests;

pub use io::{prepare_shell, write_shims};
pub use plan::{IntegrationPlan, IntegrationState, ShellStart, ShimFile, plan_integration};
pub use resolve::{Shell, ShellKind, UserId, passwd_login_shell, resolve_shell};
pub use scripts::{ShellScript, integration_scripts};
