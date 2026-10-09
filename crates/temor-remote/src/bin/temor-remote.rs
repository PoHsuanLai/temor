//! `temor-remote`: the helper temor uploads to an ssh host (PLAN §9b, Tier B).
//!
//! - `temor-remote stdio` (also the default): the end of the ssh channel. Connects to the
//!   person's daemon, starting it when needed.
//! - `temor-remote daemon`: the session daemon.
//! - `temor-remote leader PROGRAM [ARGS...]`: how the daemon starts a session's program on its
//!   terminal (not meant to be run by hand).
//! - `temor-remote version`: the helper's identity, the hash of this binary.

use std::process::ExitCode;

use temor_remote::host::{HostEnv, hash_executable, run_daemon, run_stdio};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let command = args.next();
    match command.as_deref().and_then(|command| command.to_str()) {
        None | Some("stdio") => finish(host_env().and_then(|env| run_stdio(&env))),
        Some("daemon") => finish(host_env().and_then(|env| run_daemon(&env))),
        Some("version") => finish(host_env().and_then(|env| {
            let helper = hash_executable(&env.exe)?;
            println!("{}", helper.0.to_hex());
            Ok(())
        })),
        Some("leader") => {
            let error = temor_pty::run_session_leader(args);
            // stderr is the session's terminal, so the person sees why their shell did not start.
            eprintln!("temor-remote: {error}");
            ExitCode::from(127)
        }
        Some(other) => {
            eprintln!("temor-remote: unknown command {other}");
            ExitCode::from(2)
        }
    }
}

fn host_env() -> Result<HostEnv, temor_remote::Error> {
    let exe = std::env::current_exe()
        .map_err(|error| temor_remote::Error::io("finding this program", error))?;
    Ok(HostEnv {
        xdg_runtime_dir: std::env::var_os("XDG_RUNTIME_DIR"),
        home: std::env::var_os("HOME"),
        shell: std::env::var_os("SHELL"),
        exe,
    })
}

fn finish(result: Result<(), temor_remote::Error>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("temor-remote: {error}");
            ExitCode::FAILURE
        }
    }
}
