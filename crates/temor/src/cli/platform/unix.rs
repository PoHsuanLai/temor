//! Linux and macOS: the controlling terminal at `/dev/tty` in raw mode, and `xdg-open` or `open`.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

#[cfg(target_os = "linux")]
use rustix::event::{PollFd, PollFlags, Timespec, poll};
#[cfg(target_os = "linux")]
use rustix::io::Errno;
#[cfg(target_os = "macos")]
use rustix::termios::SpecialCodeIndex;
use rustix::termios::{LocalModes, OptionalActions, Termios, tcgetattr, tcsetattr};
#[cfg(target_os = "macos")]
use std::time::Instant;
use temor_transfer::kitty::Link;

use crate::cli::error::CliError;
use crate::cli::opener::Opener;

/// What can go wrong with the terminal or the opener. Every message is for a person.
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("This only works in a terminal window")]
    NoTerminal,
    #[error("Couldn't talk to the terminal: {0}")]
    Terminal(#[source] io::Error),
    #[error("There is no program on this computer to open {}", .0.display())]
    NoOpener(PathBuf),
    #[error("Couldn't open {}: {}", .path.display(), .source)]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// The controlling terminal in raw mode, so replies are read byte for byte and nothing echoes.
/// Interrupt (Ctrl+C) still works. The old settings come back when this is dropped. One thread
/// uses it.
#[derive(Debug)]
pub struct Tty {
    file: File,
    saved: Termios,
}

/// Opens `/dev/tty`, which is the terminal the command was typed in even when its output is
/// piped, and over SSH.
pub fn open_tty() -> Result<Tty, CliError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|_| SystemError::NoTerminal)?;
    let saved = tcgetattr(&file).map_err(|errno| SystemError::Terminal(errno.into()))?;
    let mut raw = saved.clone();
    raw.make_raw();
    raw.local_modes.insert(LocalModes::ISIG);
    // macOS cannot `poll` a terminal device (it answers `POLLNVAL`), so a read there waits in the
    // terminal driver instead: it gives up after a tenth of a second with no byte (`VMIN` 0,
    // `VTIME` 1) and `read` loops until its own timeout.
    #[cfg(target_os = "macos")]
    {
        raw.special_codes[SpecialCodeIndex::VMIN] = 0;
        raw.special_codes[SpecialCodeIndex::VTIME] = 1;
    }
    tcsetattr(&file, OptionalActions::Flush, &raw)
        .map_err(|errno| SystemError::Terminal(errno.into()))?;
    Ok(Tty { file, saved })
}

impl Drop for Tty {
    fn drop(&mut self) {
        // Nothing to do if this fails: the shell resets the terminal at its next prompt.
        let _ = tcsetattr(&self.file, OptionalActions::Drain, &self.saved);
    }
}

impl Link for Tty {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes)?;
        self.file.flush()
    }

    #[cfg(target_os = "macos")]
    fn read(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let until = Instant::now() + timeout;
        let mut buf = [0_u8; 8192];
        loop {
            // With `VMIN` 0, no byte within `VTIME` reads as 0: that is a pause, not the end.
            match self.file.read(&mut buf) {
                Ok(0) if Instant::now() >= until => return Ok(None),
                Ok(0) => {}
                Ok(n) => return Ok(Some(buf[..n].to_vec())),
                Err(err) if err.kind() == io::ErrorKind::Interrupted => return Ok(None),
                Err(err) => return Err(err),
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn read(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let limit = Timespec {
            tv_sec: timeout.as_secs().try_into().unwrap_or(i64::MAX),
            tv_nsec: timeout.subsec_nanos().into(),
        };
        let mut fds = [PollFd::new(&self.file, PollFlags::IN)];
        match poll(&mut fds, Some(&limit)) {
            Ok(_) => {}
            Err(Errno::INTR) => return Ok(None),
            Err(errno) => return Err(errno.into()),
        }
        let ready = PollFlags::IN | PollFlags::HUP | PollFlags::ERR;
        if !fds[0].revents().intersects(ready) {
            return Ok(None);
        }
        let mut buf = [0_u8; 8192];
        match self.file.read(&mut buf)? {
            0 => Err(io::ErrorKind::UnexpectedEof.into()),
            n => Ok(Some(buf[..n].to_vec())),
        }
    }
}

#[cfg(target_os = "linux")]
const OPENER: &str = "xdg-open";
#[cfg(target_os = "macos")]
const OPENER: &str = "/usr/bin/open";

/// Opens files with `xdg-open`, or `open` on macOS.
#[derive(Debug, Clone, Copy)]
pub struct SystemOpener;

impl Opener for SystemOpener {
    fn open(&self, path: &Path) -> Result<(), CliError> {
        let started = Command::new(OPENER)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match started {
            Ok(_child) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Err(SystemError::NoOpener(path.to_path_buf()).into())
            }
            Err(source) => Err(SystemError::Open {
                path: path.to_path_buf(),
                source,
            }
            .into()),
        }
    }
}
