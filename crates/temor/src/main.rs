//! temor's entry point: a thin `main` over `temor-ui`. The same binary is the session leader
//! that starts a shell (`--temor-session-leader`, see temor-pty's trampoline).

mod cli;
mod run_arg;

use std::ffi::OsString;
use std::process::ExitCode;

use temor_desktop::{DesktopEnv, NOT_FOUND, Request, SystemInstance};
use temor_ui::app::desktop::{Joined, join};
use temor_ui::edge::{Ambient, load_keymap};
use temor_ui::settings::{load_settings, write_schema};
use temor_ui::{
    AppearanceSource, FirstWords, Launch, LauncherChoice, SESSION_LEADER_ARG, TakeOnce,
};

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == SESSION_LEADER_ARG) {
        // Becomes the session leader on the terminal and execs the shell; returns only on failure.
        let error = temor_pty::run_session_leader(args.into_iter().skip(2));
        eprintln!("temor: {error}");
        return ExitCode::from(u8::try_from(NOT_FOUND).unwrap_or(127));
    }
    if let Some(code) = write_schema_flag(&args) {
        return code;
    }
    if let Some(code) = cli::dispatch(&args) {
        return code;
    }
    let request =
        match run_arg::launch_request(&args, std::env::current_dir().ok(), std::env::vars_os()) {
            Ok(request) => request,
            Err(said) => {
                eprintln!("temor: {said}");
                return ExitCode::FAILURE;
            }
        };
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
    // A launcher or a notification click hands the new process a one-use token that lets the
    // running window take the keyboard on Wayland.
    let token =
        temor_ui::app::desktop::launch_token(std::env::var("XDG_ACTIVATION_TOKEN").ok().as_deref());
    // `temor -e` lives as long as its tab, as a terminal's process does: the program runs in the
    // temor that is running (or that the bus starts), and this launch exits with its status once
    // the tab closes. With no temor to reach, this launch is the window.
    if let Request::Run(run) = &request {
        let instance = SystemInstance::new(DesktopEnv::from_process());
        match runtime.block_on(instance.run_and_wait(run, token.as_deref())) {
            Ok(Some(status)) => return ExitCode::from(u8::try_from(status).unwrap_or(1)),
            Ok(None) => {}
            Err(err) => {
                eprintln!("temor: {err}");
                return ExitCode::FAILURE;
            }
        }
    }
    let desktop = match join(
        runtime.handle(),
        DesktopEnv::from_process(),
        &request,
        token.as_deref(),
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
    let loaded = match load_settings(&ambient) {
        Ok(loaded) => loaded,
        Err(err) => {
            eprintln!("temor: {err}");
            return ExitCode::FAILURE;
        }
    };
    for note in &loaded.notes {
        eprintln!("temor: {note}");
    }
    // The keys: the desktop's conventions, then the person's changes, then temor's own actions.
    let (keymap, key_issues) = load_keymap(&loaded.settings.keys, &ambient);
    for (line, issue) in &key_issues {
        match *line {
            0 => eprintln!("temor: keys: {issue}"),
            line => eprintln!("temor: keys.conf line {line}: {issue}"),
        }
    }
    let mut told = loaded.told;
    told.key_issues(&key_issues);
    // The session file and its snapshots live under the XDG state home (a relative value is not used).
    let state_dir = ambient.state_dir();
    let launched = temor_ui::app::run_with(
        Launch {
            ambient,
            settings: loaded.settings,
            keymap,
            launcher: LauncherChoice::ThisProgram,
            appearance: AppearanceSource::desktop(),
            probe: None,
            state_dir,
            first_words: FirstWords::new(told.sentences()),
            first_run: TakeOnce::new(match request {
                Request::Run(run) => Some(run.into()),
                _ => None,
            }),
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

/// `temor --write-schema <dir>`: writes `temor.settings.toml`, the schema a Settings page draws
/// temor's settings from, into `dir` and ends. `None` for any other command line.
fn write_schema_flag(args: &[OsString]) -> Option<ExitCode> {
    if args.get(1)? != "--write-schema" {
        return None;
    }
    let written = match args {
        [_, _, dir] => write_schema(std::path::Path::new(dir)),
        _ => Err("--write-schema takes one folder to write the schema into".to_owned()),
    };
    Some(match written {
        Ok(said) => {
            print!("{said}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("temor: {message}");
            ExitCode::FAILURE
        }
    })
}
