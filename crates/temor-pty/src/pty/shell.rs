//! The one-call path from a shell to the [`SpawnSpec`] that starts it with integration.

use std::path::{Path, PathBuf};

use temor_shell::{ChildEnv, Shell, ShellStart, plan_integration, write_shims};

use super::model::SpawnSpec;
use super::user::current_user;
use crate::error::PtyError;
use crate::size::GridSize;

/// Plans the integration for `shell`, writes its shims under `runtime_dir`, and returns what
/// to spawn in `cwd` at `size`.
pub fn prepare_shell(
    shell: &Shell,
    start: ShellStart,
    env: ChildEnv,
    runtime_dir: &Path,
    cwd: PathBuf,
    size: GridSize,
) -> Result<SpawnSpec, PtyError> {
    let plan = plan_integration(shell, start, env, runtime_dir);
    write_shims(&plan.files, current_user())?;
    Ok(SpawnSpec {
        program: plan.program,
        args: plan.args,
        cwd,
        env: plan.env,
        size,
    })
}
