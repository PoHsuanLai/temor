//! `temor-pty-exec PROGRAM [ARGS...]`: become a session leader on the terminal at stdin, then
//! run PROGRAM. temor starts this through `SessionLauncher::Trampoline`; it is not meant to be
//! run by hand.

use std::process::ExitCode;

fn main() -> ExitCode {
    let error = temor_pty::run_session_leader(std::env::args_os().skip(1));
    // stderr is the new terminal, so the person sees why their shell did not start.
    eprintln!("temor: {error}");
    ExitCode::from(127)
}
