//! temor's entry point: a thin `main` over `temor-ui`. The same binary is the session leader
//! that starts a shell (`--temor-session-leader`, see temor-pty's trampoline).

use std::ffi::OsString;
use std::process::ExitCode;

use temor_ui::edge::{Ambient, load_settings};
use temor_ui::{AppearanceSource, Launch, LauncherChoice, SESSION_LEADER_ARG};

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == SESSION_LEADER_ARG) {
        // Becomes the session leader on the terminal and execs the shell; returns only on failure.
        let error = temor_pty::run_session_leader(args.into_iter().skip(2));
        eprintln!("temor: {error}");
        return ExitCode::from(127);
    }
    let Some(ambient) = Ambient::from_process() else {
        eprintln!("temor: there is no home directory to start in");
        return ExitCode::FAILURE;
    };
    let parsed = match load_settings(&ambient.settings_path()) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("temor: {err}");
            return ExitCode::FAILURE;
        }
    };
    for (line, issue) in &parsed.issues {
        eprintln!("temor: settings line {line}: {issue}");
    }
    let launched = temor_ui::run(Launch {
        ambient,
        settings: parsed.settings,
        launcher: LauncherChoice::ThisProgram,
        appearance: AppearanceSource::desktop(),
        probe: None,
    });
    match launched {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("temor: {err}");
            ExitCode::FAILURE
        }
    }
}
