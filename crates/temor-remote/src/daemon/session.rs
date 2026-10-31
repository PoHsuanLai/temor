//! One session: a program on a terminal, and the screen state libghostty keeps for it.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use temor_pty::{ChildExit, Pty, PtyIo};
use temor_vt::{Terminal, TerminalConfig};
use temor_wire::ids::{RemoteSessionId, RequestId};
use temor_wire::link::Monotonic;
use temor_wire::message::{
    Event, Message, OpenSession, Palette, RemotePath, Reply, SessionInfo, SessionState,
};
use temor_wire::units::{CellPx as CellSize, Size};

use super::colours::apply_palette;
use super::config::DaemonConfig;
use super::exit::exit_status;
use super::input::Input;
use super::launch::spawn_spec;
use super::link::{Link, LinkId};
use super::sinks::{CwdSink, Replies};
use super::sizes::{check, pty_size, screen_cell, screen_size, usable};
use crate::clock::Clock;
use crate::error::Error;
use crate::sync::lock;

/// How long a program may take to leave after its terminal hangs up, and how often that is polled.
const GRACE: Duration = Duration::from_millis(500);
/// How long a killed program is polled (its output drained) before the wait blocks.
const KILLED: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(20);

/// A session. `Send` and `Sync`: the pump thread and the connections share it.
#[derive(Debug)]
pub(super) struct Session {
    pub(super) id: RemoteSessionId,
    shared: Mutex<Shared>,
    pty: Mutex<Pty>,
    input: Arc<Input>,
    cwd: Arc<Mutex<String>>,
    clock: Arc<dyn Clock>,
}

#[derive(Debug)]
struct Shared {
    screen: Terminal,
    size: Size,
    /// Every client attached: each gets the output, so several windows can show one session.
    subscribers: Vec<Arc<Link>>,
    detached_at: Option<Monotonic>,
    /// A client asked to close the session. A hang-up alone can be lost (a program that has
    /// not yet made its own process group misses it), so the pump also ends on this and reaps.
    close_asked: bool,
}

impl Session {
    /// Starts the program and its screen. The session begins detached. The terminal's read end is
    /// returned for the pump thread.
    pub(super) fn start(
        id: RemoteSessionId,
        request: &OpenSession,
        config: &DaemonConfig,
    ) -> Result<(Arc<Self>, PtyIo), Error> {
        let size = check(request.size)?;
        let cell = usable(request.cell);
        let program = request.shell.as_ref().map_or_else(
            || config.default_shell.clone(),
            |path| PathBuf::from(&path.0),
        );
        let spec = spawn_spec(
            program,
            &request.env,
            &config.runtime_dir,
            PathBuf::from(&request.cwd.0),
            pty_size(size, cell),
        )?;
        let pty = Pty::spawn(&spec, &config.launcher)?;
        let reader = pty.io()?;
        let input = Arc::new(Input::new(pty.io()?));
        let cwd = Arc::new(Mutex::new(request.cwd.0.clone()));
        let mut screen_config = TerminalConfig::new(screen_size(size));
        screen_config.cell_px = screen_cell(cell);
        let mut screen = Terminal::new(
            screen_config,
            Box::new(CwdSink(Arc::clone(&cwd))),
            Box::new(Replies(Arc::clone(&input))),
        )?;
        if let Some(palette) = &request.palette {
            apply_palette(&mut screen, palette)?;
        }
        let session = Arc::new(Self {
            id,
            shared: Mutex::new(Shared {
                screen,
                size,
                subscribers: Vec::new(),
                detached_at: Some(config.clock.now()),
                close_asked: false,
            }),
            pty: Mutex::new(pty),
            input,
            cwd,
            clock: Arc::clone(&config.clock),
        });
        Ok((session, reader))
    }

    /// Feeds program output to the screen and on to every attached client.
    pub(super) fn output(&self, bytes: &[u8]) {
        let mut shared = self.lock();
        shared.screen.write(bytes);
        if shared.subscribers.is_empty() {
            return;
        }
        let event = Message::Event(Event::Output { session: self.id });
        // A client that is gone or too slow is dropped; it reattaches for a snapshot.
        shared
            .subscribers
            .retain(|link| link.try_send(&event, Some(bytes)).is_ok());
        if shared.subscribers.is_empty() {
            shared.detached_at = Some(self.clock.now());
        }
    }

    pub(super) fn flush_input(&self) -> bool {
        self.input.flush()
    }

    pub(super) fn backlog(&self) -> bool {
        self.input.backlog()
    }

    pub(super) fn send_input(&self, bytes: &[u8]) {
        self.input.send(bytes);
    }

    /// Answers an attach request with the screen's snapshot, and from then on sends the live
    /// output to `link` too: the clients already attached stay. Both happen under the session's
    /// lock, so no output falls between them. A link that attaches again is counted once.
    pub(super) fn attach(&self, link: &Arc<Link>, request: RequestId) -> Result<(), Error> {
        let mut shared = self.lock();
        let snapshot = shared.screen.snapshot()?;
        let reply = Message::Response {
            id: request,
            outcome: Ok(Reply::Attached {
                session: self.id,
                size: shared.size,
            }),
        };
        link.send(&reply, Some(snapshot.as_bytes()))?;
        shared.subscribers.retain(|attached| attached.id != link.id);
        shared.subscribers.push(Arc::clone(link));
        shared.detached_at = None;
        Ok(())
    }

    /// Detaches `from` (every client when `from` is `None`); the others stay attached.
    pub(super) fn detach(&self, from: Option<LinkId>) {
        let mut shared = self.lock();
        let before = shared.subscribers.len();
        shared
            .subscribers
            .retain(|link| from.is_some_and(|id| link.id != id));
        if before > 0 && shared.subscribers.is_empty() {
            shared.detached_at = Some(self.clock.now());
        }
    }

    /// Changes the size of the screen and the program's terminal together.
    pub(super) fn resize(&self, size: Size, cell: CellSize) -> Result<(), Error> {
        let size = check(size)?;
        let cell = usable(cell);
        let mut shared = self.lock();
        shared.screen.resize(screen_size(size), screen_cell(cell))?;
        self.input.resize(pty_size(size, cell))?;
        shared.size = size;
        Ok(())
    }

    /// Sets the colours the screen answers colour queries with.
    pub(super) fn set_palette(&self, palette: &Palette) -> Result<(), Error> {
        apply_palette(&mut self.lock().screen, palette)
    }

    /// Closes the session for a client: hangs the program up, and has the pump end and reap it
    /// (hang-up, grace period, kill) even if the hang-up was missed.
    pub(super) fn close(&self) {
        self.lock().close_asked = true;
        self.hang_up();
    }

    /// Whether a client asked to close the session.
    pub(super) fn close_asked(&self) -> bool {
        self.lock().close_asked
    }

    /// Hangs up the program, as closing a terminal window would. The pump thread reaps it.
    pub(super) fn hang_up(&self) {
        // Already gone is fine.
        let _ = self.pty().hang_up();
    }

    pub(super) fn info(&self) -> SessionInfo {
        let shared = self.lock();
        let cwd = lock(&self.cwd).clone();
        SessionInfo {
            session: self.id,
            cwd: RemotePath(cwd),
            size: shared.size,
            state: if !shared.subscribers.is_empty() {
                SessionState::Attached
            } else {
                SessionState::Detached
            },
        }
    }

    /// The program's process id.
    pub(super) fn pid(&self) -> u32 {
        u32::try_from(self.pty().pid().0).unwrap_or(0)
    }

    /// When the last client left, if none is attached.
    pub(super) fn detached_since(&self) -> Option<Monotonic> {
        self.lock().detached_at
    }

    /// Waits for the program to leave after its terminal closed: hangs it up, gives it the grace
    /// period, then kills it. `None` when its status is no longer known.
    ///
    /// `drain` runs on every poll, after the kill too: a program exiting on a terminal nobody
    /// reads can stall in its own exit (macOS holds the process there until the output is taken),
    /// so a blocking wait could wait for ever, holding the terminal's lock.
    pub(super) fn reap(&self, mut drain: impl FnMut()) -> Option<ChildExit> {
        self.hang_up();
        let mut waited = Duration::ZERO;
        loop {
            {
                let mut pty = self.pty();
                if let Ok(Some(exit)) = pty.try_exit() {
                    return Some(exit);
                }
                if waited >= GRACE + KILLED {
                    return pty.kill().ok();
                }
                if waited >= GRACE {
                    let _ = pty.signal_group(temor_pty::SignalKind::Kill);
                }
            }
            drain();
            std::thread::sleep(POLL);
            waited += POLL;
        }
    }

    /// Tells every attached client the program ended.
    pub(super) fn exited(&self, exit: Option<ChildExit>) {
        let links = std::mem::take(&mut self.lock().subscribers);
        let event = Message::Event(Event::Exited {
            session: self.id,
            status: exit_status(exit),
        });
        for link in links {
            // A client that cannot take it has gone; there is no one to tell.
            let _ = link.try_send(&event, None);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        lock(&self.shared)
    }

    fn pty(&self) -> MutexGuard<'_, Pty> {
        lock(&self.pty)
    }
}
