//! The entry from `main`: is this one of ours, and if so, run it.

use std::ffi::OsString;
use std::process::ExitCode;

use super::args::{Command, parse};
use super::error::CliError;
use super::exec::{Context, run};
use super::platform::SystemOpener;
use super::text::USAGE;
use super::view::where_from;

/// Runs a subcommand if the first word is one. `None` leaves the window to start as usual.
pub fn dispatch(args: &[OsString]) -> Option<ExitCode> {
    let parsed = parse(args)?;
    let command = match parsed {
        Ok(Command::Help) => {
            println!("{USAGE}");
            return Some(ExitCode::SUCCESS);
        }
        Ok(Command::Version) => {
            println!("temor {}", env!("CARGO_PKG_VERSION"));
            return Some(ExitCode::SUCCESS);
        }
        Ok(command) => command,
        Err(usage) => return Some(fail(&CliError::Usage(usage))),
    };
    let context = Context {
        at: where_from(
            std::env::var_os("SSH_CONNECTION").as_deref(),
            std::env::var_os("SSH_TTY").as_deref(),
        ),
        pid: std::process::id(),
    };
    match run(command, &context, &SystemOpener) {
        Ok(said) => {
            println!("{said}");
            Some(ExitCode::SUCCESS)
        }
        Err(err) => Some(fail(&err)),
    }
}

fn fail(err: &CliError) -> ExitCode {
    eprintln!("{err}");
    if let CliError::Usage(_) = err {
        eprintln!("{USAGE}");
    }
    ExitCode::FAILURE
}
