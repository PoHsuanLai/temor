//! How a session's program is started: as a login shell with temor's shell integration, the same
//! plan local panes use, so the window gets prompt marks and folder reports from the host.

use std::path::{Path, PathBuf};

use temor_pty::{GridSize, SpawnSpec, current_user};
use temor_shell::{ChildEnv, IntegrationPlan, Shell, ShellStart, plan_integration, write_shims};
use temor_wire::message::EnvVar;

use crate::error::Error;

/// The request's environment as the child's base. The helper adds nothing of its own.
fn base_env(vars: &[EnvVar]) -> ChildEnv {
    let mut env = ChildEnv::new();
    for var in vars {
        env.set(&var.name, &var.value);
    }
    env
}

/// The plan for starting `program` with the request's environment. A login start, so the account's
/// profile runs as it would over ssh. `TEMOR_SHELL_INTEGRATION=0` in the environment, or a
/// program that is not a shell temor knows, starts it plainly.
fn plan(program: &Path, vars: &[EnvVar], runtime_dir: &Path) -> IntegrationPlan {
    plan_integration(
        &Shell::at(program),
        ShellStart::Login,
        base_env(vars),
        runtime_dir,
    )
}

/// What to spawn for the request: lays the shims down under `runtime_dir` (rewriting them is
/// harmless) and returns the program, arguments and environment.
pub(super) fn spawn_spec(
    program: PathBuf,
    vars: &[EnvVar],
    runtime_dir: &Path,
    cwd: PathBuf,
    size: GridSize,
) -> Result<SpawnSpec, Error> {
    let plan = plan(&program, vars, runtime_dir);
    write_shims(&plan.files, current_user()).map_err(temor_pty::PtyError::from)?;
    Ok(SpawnSpec {
        program: plan.program,
        args: plan.args,
        cwd,
        env: plan.env,
        size,
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    fn var(name: &str, value: &str) -> EnvVar {
        EnvVar {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    fn args(plan: &IntegrationPlan) -> Vec<String> {
        plan.args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_known_shell_gets_the_integration_and_the_rest_a_plain_login() {
        let run = Path::new("/run/temor");
        let cases = [
            ("/bin/bash", vec![], true, "bash starts through the shim rc"),
            (
                "/bin/bash",
                vec![var("TEMOR_SHELL_INTEGRATION", "0")],
                false,
                "the opt-out starts it plainly",
            ),
            ("/bin/dash", vec![], false, "no script for dash"),
        ];
        for (program, vars, integrated, why) in cases {
            let plan = plan(Path::new(program), &vars, run);
            assert_eq!(!plan.files.is_empty(), integrated, "{why}");
            if !integrated {
                assert_eq!(args(&plan), ["-l"], "{why}");
            }
        }
    }

    #[test]
    fn the_request_env_is_the_base_and_the_terminal_name_is_kept() {
        let vars = [var("TERM", "xterm-kitty"), var("HOME", "/h")];
        let plan = plan(Path::new("/bin/bash"), &vars, Path::new("/run/temor"));
        let get = |key| plan.env.get(key).map(std::ffi::OsStr::to_os_string);
        assert_eq!(get("TERM"), Some(OsString::from("xterm-kitty")));
        assert_eq!(get("HOME"), Some(OsString::from("/h")));
        assert_eq!(get("TEMOR_BASH_LOGIN"), Some(OsString::from("1")));
    }
}
