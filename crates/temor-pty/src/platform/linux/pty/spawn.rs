//! Opening the pair and starting the child on it.

use std::os::fd::{AsFd, OwnedFd};
use std::process::{Child, Command, Stdio};

use rustix::fs::{Mode, OFlags};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{InputModes, OptionalActions, Winsize, tcgetattr, tcsetattr, tcsetwinsize};

use super::io::PtyIo;
use crate::error::PtyError;
use crate::pty::SessionLauncher;
use crate::pty::{ChildExit, ProcessId, SpawnSpec};
use crate::size::GridSize;

/// Whether the child has been reaped. Once it has, its pid (and so its process group id) may
/// be reused, and the group is never signalled again.
#[derive(Debug)]
pub(super) enum ChildState {
    Running,
    Reaped(ChildExit),
}

/// A pseudo-terminal with a program running on it as a session leader. Dropping it hangs up
/// the program's process group and closes the terminal, without waiting (see `Drop`).
#[derive(Debug)]
pub struct Pty {
    pub(super) master: OwnedFd,
    pub(super) child: Child,
    pub(super) state: ChildState,
}

impl Pty {
    /// Opens a terminal at `spec.size` and starts `spec.program` on it through `launcher`, in
    /// `spec.cwd`, with exactly `spec.env`.
    pub fn spawn(spec: &SpawnSpec, launcher: &SessionLauncher) -> Result<Self, PtyError> {
        let (master, slave) = open_pair().map_err(PtyError::Open)?;
        set_size(&master, spec.size).map_err(PtyError::Open)?;
        set_utf8(&master).map_err(PtyError::Open)?;
        let mut command = launch_command(spec, launcher);
        command
            .env_clear()
            .envs(spec.env.iter())
            .current_dir(&spec.cwd)
            .stdin(Stdio::from(slave.try_clone().map_err(PtyError::Open)?))
            .stdout(Stdio::from(slave.try_clone().map_err(PtyError::Open)?))
            .stderr(Stdio::from(slave));
        let child = command.spawn().map_err(|source| PtyError::Spawn {
            program: command.get_program().into(),
            source,
        })?;
        // `command` still holds the slave descriptors; they must close here, or the master
        // never sees the child's end of the terminal close.
        drop(command);
        Ok(Self {
            master,
            child,
            state: ChildState::Running,
        })
    }

    /// A nonblocking handle on the terminal's master side, for the caller's I/O thread.
    pub fn io(&self) -> Result<PtyIo, PtyError> {
        PtyIo::new(&self.master)
    }

    /// Tells the program its new size (`TIOCSWINSZ`; the kernel sends `SIGWINCH`).
    pub fn resize(&self, size: GridSize) -> Result<(), PtyError> {
        set_size(&self.master, size).map_err(PtyError::Resize)
    }

    /// The program's pid, which is also its process group and session id.
    pub fn pid(&self) -> ProcessId {
        // A pid always fits in i32 (`pid_t`); std hands it out as u32.
        ProcessId(self.child.id().try_into().unwrap_or(i32::MAX))
    }
}

fn open_pair() -> std::io::Result<(OwnedFd, OwnedFd)> {
    let master = open_master()?;
    grantpt(&master)?;
    unlockpt(&master)?;
    let name = ptsname(&master, Vec::new())?;
    let slave = rustix::fs::open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    Ok((master, slave))
}

fn open_master() -> std::io::Result<OwnedFd> {
    Ok(openpt(
        OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC,
    )?)
}

/// Tells the line discipline input is UTF-8 (`IUTF8`), so backspace in canonical mode erases a
/// whole character rather than one byte of it.
fn set_utf8(master: &OwnedFd) -> std::io::Result<()> {
    let mut termios = tcgetattr(master)?;
    termios.input_modes.insert(InputModes::IUTF8);
    tcsetattr(master, OptionalActions::Now, &termios)?;
    Ok(())
}

fn launch_command(spec: &SpawnSpec, launcher: &SessionLauncher) -> Command {
    match launcher {
        SessionLauncher::Setsid(setsid) => {
            let mut command = Command::new(setsid);
            command.args(["--ctty", "--"]);
            command.arg(&spec.program).args(&spec.args);
            command
        }
        SessionLauncher::Trampoline { program, args } => {
            let mut command = Command::new(program);
            command.args(args).arg(&spec.program).args(&spec.args);
            command
        }
    }
}

pub(super) fn set_size(fd: impl AsFd, size: GridSize) -> std::io::Result<()> {
    let winsize = Winsize {
        ws_row: size.rows.0,
        ws_col: size.cols.0,
        ws_xpixel: size.width.0,
        ws_ypixel: size.height.0,
    };
    tcsetwinsize(fd, winsize)?;
    Ok(())
}
