//! `PtyIo`: nonblocking reads and writes on the master side, driven by the caller's thread.

use std::io::{PipeReader, PipeWriter, Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::Errno;

use super::spawn::set_size;
use crate::error::PtyError;
use crate::pty::{Interest, ReadOutcome, Readiness, Ready, WriteOutcome};
use crate::size::GridSize;

/// The master side of a terminal, nonblocking. It is `Send`, so the caller moves it to the
/// pane's I/O thread; it also implements `AsFd`, for callers with their own poller.
#[derive(Debug)]
pub struct PtyIo {
    fd: OwnedFd,
}

impl PtyIo {
    /// Tells the program its new size (`TIOCSWINSZ` on the shared master; the kernel sends
    /// `SIGWINCH`). Callers that reflow their own terminal do this under the same lock.
    pub fn resize(&self, size: GridSize) -> Result<(), PtyError> {
        set_size(&self.fd, size).map_err(PtyError::Resize)
    }

    pub(super) fn new(master: &OwnedFd) -> Result<Self, PtyError> {
        let fd = master.try_clone().map_err(PtyError::Io)?;
        // The flag lives on the shared open file, so the `Pty`'s own descriptor is
        // nonblocking too; it only ever makes ioctls.
        let flags = fcntl_getfl(&fd).map_err(|e| PtyError::Io(e.into()))?;
        fcntl_setfl(&fd, flags | OFlags::NONBLOCK).map_err(|e| PtyError::Io(e.into()))?;
        Ok(Self { fd })
    }

    /// Reads what the program wrote, without blocking.
    pub fn read(&self, buf: &mut [u8]) -> Result<ReadOutcome, PtyError> {
        loop {
            return match rustix::io::read(&self.fd, &mut *buf) {
                Ok(0) => Ok(ReadOutcome::Closed),
                Ok(n) => Ok(ReadOutcome::Data(n)),
                Err(Errno::INTR) => continue,
                Err(Errno::AGAIN) => Ok(ReadOutcome::WouldBlock),
                // Linux reports a terminal whose other side has all closed as EIO (macOS reads 0).
                Err(Errno::IO) => Ok(ReadOutcome::Closed),
                Err(errno) => Err(PtyError::Io(errno.into())),
            };
        }
    }

    /// Writes input for the program, without blocking; the caller keeps what was not taken.
    pub fn write(&self, bytes: &[u8]) -> Result<WriteOutcome, PtyError> {
        loop {
            return match rustix::io::write(&self.fd, bytes) {
                Ok(n) => Ok(WriteOutcome::Wrote(n)),
                Err(Errno::INTR) => continue,
                Err(Errno::AGAIN) => Ok(WriteOutcome::WouldBlock),
                Err(Errno::IO) => Ok(WriteOutcome::Closed),
                Err(errno) => Err(PtyError::Io(errno.into())),
            };
        }
    }

    /// Blocks until a direction in `interest` can progress, or `timeout` passes (`None` waits
    /// forever). A hangup counts as readable: the next read reports [`ReadOutcome::Closed`].
    pub fn wait(
        &self,
        interest: Interest,
        timeout: Option<Duration>,
    ) -> Result<Readiness, PtyError> {
        self.wait_on(interest, None, timeout)
    }

    /// [`wait`](Self::wait), ending early too when `wake` is woken (from another thread, by one
    /// of its [`Waker`]s). The wakes it took are used up.
    pub fn wait_or(
        &self,
        interest: Interest,
        wake: &Wake,
        timeout: Option<Duration>,
    ) -> Result<Readiness, PtyError> {
        let readiness = self.wait_on(interest, Some(wake.read.as_fd()), timeout)?;
        wake.take();
        Ok(readiness)
    }

    fn wait_on(
        &self,
        interest: Interest,
        also: Option<BorrowedFd<'_>>,
        timeout: Option<Duration>,
    ) -> Result<Readiness, PtyError> {
        let events = match interest {
            Interest::Read => PollFlags::IN,
            Interest::Write => PollFlags::OUT,
            Interest::ReadWrite => PollFlags::IN | PollFlags::OUT,
        };
        let mut fds = vec![PollFd::new(&self.fd, events)];
        if let Some(fd) = &also {
            fds.push(PollFd::new(fd, PollFlags::IN));
        }
        let timeout = timeout.map(timespec);
        match poll(&mut fds, timeout.as_ref()) {
            Ok(_) | Err(Errno::INTR) => {}
            Err(errno) => return Err(PtyError::Io(errno.into())),
        }
        let revents = fds[0].revents();
        let read = match events.contains(PollFlags::IN) {
            true => ready(revents, PollFlags::IN | PollFlags::HUP | PollFlags::ERR),
            false => Ready::Later,
        };
        Ok(Readiness {
            read,
            // A hangup makes the next write report `Closed` rather than block.
            write: ready(revents, PollFlags::OUT | PollFlags::HUP | PollFlags::ERR),
        })
    }
}

/// What ends a [`PtyIo::wait_or`] early: a pipe whose writing end its [`Waker`]s share.
#[derive(Debug)]
pub struct Wake {
    read: PipeReader,
    write: Arc<PipeWriter>,
}

/// Wakes the thread waiting on its [`Wake`]. `Send`, `Sync` and cheap to clone.
#[derive(Debug, Clone)]
pub struct Waker(Arc<PipeWriter>);

impl Wake {
    pub fn new() -> Result<Self, PtyError> {
        let (read, write) = std::io::pipe().map_err(PtyError::Io)?;
        for fd in [read.as_fd(), write.as_fd()] {
            let flags = fcntl_getfl(fd).map_err(|e| PtyError::Io(e.into()))?;
            fcntl_setfl(fd, flags | OFlags::NONBLOCK).map_err(|e| PtyError::Io(e.into()))?;
        }
        Ok(Self {
            read,
            write: Arc::new(write),
        })
    }

    pub fn waker(&self) -> Waker {
        Waker(Arc::clone(&self.write))
    }

    /// Empties the pipe, so the next wait blocks until the next wake.
    fn take(&self) {
        let mut buf = [0u8; 64];
        while matches!((&self.read).read(&mut buf), Ok(n) if n > 0) {}
    }
}

impl Waker {
    pub fn wake(&self) {
        // A full pipe already wakes the waiter.
        let _ = (&*self.0).write(&[1]);
    }
}

fn ready(revents: PollFlags, any_of: PollFlags) -> Ready {
    match revents.intersects(any_of) {
        true => Ready::Now,
        false => Ready::Later,
    }
}

fn timespec(duration: Duration) -> Timespec {
    Timespec {
        tv_sec: duration.as_secs().try_into().unwrap_or(i64::MAX),
        tv_nsec: duration.subsec_nanos().into(),
    }
}

impl AsFd for PtyIo {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}
