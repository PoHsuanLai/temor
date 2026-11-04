//! The one way temor hands something to the desktop's own opener: a file to the program the
//! desktop chose for it, a file to the file manager, a checked link to the browser. macOS has
//! `/usr/bin/open`; Linux the desktop portal's `OpenURI` for a link when a session bus is there,
//! and `xdg-open`. Every opener started is waited for on a thread of its own, so it never stays a
//! zombie and the caller never waits on it.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::env::DesktopEnv;

/// The desktop's opener.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemOpen;

impl SystemOpen {
    /// Opens the file at `path` with the program the desktop chose for it. An opener that is not
    /// there is [`io::ErrorKind::NotFound`]; a system with none, [`io::ErrorKind::Unsupported`].
    pub fn open(path: &Path) -> io::Result<()> {
        run(path.as_os_str(), false)
    }

    /// Shows the file at `path` in the file manager: selected in Finder; its folder opened on
    /// Linux.
    pub fn reveal(path: &Path) -> io::Result<()> {
        if cfg!(target_os = "macos") {
            run(path.as_os_str(), true)
        } else {
            // TODO(platform:linux): org.freedesktop.FileManager1.ShowItems selects the file; the
            // folder is the fallback every desktop has.
            run(path.parent().unwrap_or(path).as_os_str(), false)
        }
    }

    /// Hands `link`, checked by the caller, to the browser. On Linux with a session bus the
    /// portal is asked first, off the caller's thread, and `xdg-open` takes the link when it
    /// does not.
    pub fn open_link(env: &DesktopEnv, link: &str) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        if env.session != crate::env::BusRoute::Absent {
            let (env, link) = (env.clone(), link.to_owned());
            std::thread::Builder::new()
                .name("temor-open".to_owned())
                .spawn(move || {
                    if !through_portal(&env, &link)
                        && let Ok(mut child) = start(OsStr::new(&link), false)
                    {
                        let _ = child.wait();
                    }
                })?;
            return Ok(());
        }
        let _ = env;
        run(OsStr::new(link), false)
    }
}

/// Starts the opener on `target` and waits for it on a thread of its own.
fn run(target: &OsStr, reveal: bool) -> io::Result<()> {
    let mut child = start(target, reveal)?;
    // A thread that cannot start leaves the opener to be reaped when temor ends.
    let _ = std::thread::Builder::new()
        .name("temor-open".to_owned())
        .spawn(move || {
            let _ = child.wait();
        });
    Ok(())
}

fn start(target: &OsStr, reveal: bool) -> io::Result<Child> {
    let mut command = if cfg!(target_os = "macos") {
        let mut command = Command::new("/usr/bin/open");
        if reveal {
            command.arg("-R");
        }
        // `--` keeps a target from being read as an option.
        command.arg("--");
        command
    } else if cfg!(target_os = "linux") {
        Command::new("xdg-open")
    } else {
        // TODO(platform:windows): ShellExecute and `explorer /select`.
        return Err(io::ErrorKind::Unsupported.into());
    };
    command
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// Whether the portal took `link`. The call runs on a runtime of its own, on this thread.
#[cfg(target_os = "linux")]
fn through_portal(env: &DesktopEnv, link: &str) -> bool {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return false;
    };
    runtime.block_on(async {
        match crate::SystemLinks::connect(env).await {
            Ok(links) => links.open(link).await.is_ok(),
            Err(_) => false,
        }
    })
}
