//! `temor -e <program> [arguments…]`: run a program in a tab of its own in place of the shell.
//! This is the terminal-exec convention the desktop entry declares (`X-TerminalArgExec=-e`), the
//! one `xdg-terminal-exec` and a desktop's "open in terminal" use. Every word after `-e` is the
//! program's, even one that looks like an option.

use std::ffi::OsString;
use std::path::PathBuf;

use temor_desktop::{Request, RunRequest};

/// What this launch asks for: a window, or a program run in `cwd` with `env`. `Err` is what to
/// tell the person when `-e` names no program.
pub fn launch_request(
    args: &[OsString],
    cwd: Option<PathBuf>,
    env: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<Request, &'static str> {
    if args.get(1).is_none_or(|arg| arg != "-e") {
        return Ok(Request::NewWindow);
    }
    let argv = args[2..].to_vec();
    if argv.first().is_none_or(|program| program.is_empty()) {
        return Err("-e needs a program to run, like: temor -e htop");
    }
    Ok(Request::Run(RunRequest {
        argv,
        cwd,
        env: env.into_iter().collect(),
        ticket: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<OsString> {
        all.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_words_after_dash_e_are_the_programs() {
        let env = vec![(OsString::from("A"), OsString::from("1"))];
        let got = launch_request(
            &words(&["temor", "-e", "claude", "--resume", "-e"]),
            Some(PathBuf::from("/work")),
            env.clone(),
        );
        assert_eq!(
            got,
            Ok(Request::Run(RunRequest {
                argv: words(&["claude", "--resume", "-e"]),
                cwd: Some(PathBuf::from("/work")),
                env,
                ticket: None,
            }))
        );
    }

    #[test]
    fn no_dash_e_is_a_window_and_dash_e_alone_is_refused() {
        assert_eq!(
            launch_request(&words(&["temor"]), None, []),
            Ok(Request::NewWindow)
        );
        assert_eq!(
            launch_request(&words(&["temor", "somewhere"]), None, []),
            Ok(Request::NewWindow)
        );
        assert!(launch_request(&words(&["temor", "-e"]), None, []).is_err());
        assert!(launch_request(&words(&["temor", "-e", ""]), None, []).is_err());
    }
}
