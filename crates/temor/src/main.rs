//! temor's entry point: a thin `main` over `temor-ui`. The same binary is the session leader
//! that starts a shell (`--temor-session-leader`, see temor-pty's trampoline).

mod cli;

use std::ffi::OsString;
use std::process::ExitCode;

use temor_desktop::{DesktopEnv, Request};
use temor_ui::app::desktop::{Joined, join};
use temor_ui::edge::{Ambient, load_keymap, load_settings, resolve_state_dir};
use temor_ui::{AppearanceSource, Launch, LauncherChoice, SESSION_LEADER_ARG};

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == SESSION_LEADER_ARG) {
        // Becomes the session leader on the terminal and execs the shell; returns only on failure.
        let error = temor_pty::run_session_leader(args.into_iter().skip(2));
        eprintln!("temor: {error}");
        return ExitCode::from(127);
    }
    if let Some(code) = cli::dispatch(&args) {
        return code;
    }
    // The bus side runs on its own small runtime, kept until the window closes. A second launch
    // hands its request to the first temor and ends here, before reading any file.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("temor: {err}");
            return ExitCode::FAILURE;
        }
    };
    let desktop = match join(
        runtime.handle(),
        DesktopEnv::from_process(),
        &Request::NewWindow,
        // A launcher or a notification click hands the new process a one-use token that lets
        // the running window take the keyboard on Wayland.
        std::env::var("XDG_ACTIVATION_TOKEN").ok().as_deref(),
    ) {
        Joined::Handed => return ExitCode::SUCCESS,
        Joined::First { desktop, trouble } => {
            if let Some(trouble) = trouble {
                eprintln!("temor: the desktop's services are off: {trouble}");
            }
            desktop
        }
    };
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
    // The keys: the desktop's conventions, then the person's changes, then temor's own actions.
    let (keymap, key_issues) = load_keymap(&parsed.settings.keys);
    for (line, issue) in parsed.issues.iter().chain(&key_issues) {
        match *line {
            0 => eprintln!("temor: keys: {issue}"),
            line => eprintln!("temor: settings line {line}: {issue}"),
        }
    }
    // The session file and its snapshots live under the XDG state home (a relative value is not used).
    let state_dir = resolve_state_dir(
        ambient.env.get("XDG_STATE_HOME").map(std::path::Path::new),
        Some(&ambient.home),
    );
    let launched = temor_ui::app::run_with(
        Launch {
            ambient,
            settings: parsed.settings,
            keymap,
            launcher: LauncherChoice::ThisProgram,
            appearance: AppearanceSource::desktop(),
            probe: None,
            state_dir,
        },
        desktop,
    );
    match launched {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("temor: {err}");
            ExitCode::FAILURE
        }
    }
}
