//! One session: a program on a terminal, and the screen state libghostty keeps for it.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use temor_pty::{ChildExit, Pty, PtyIo, SpawnSpec};
use temor_shell::ChildEnv;
use temor_vt::{Terminal, TerminalConfig};
use temor_wire::ids::{RequestId, SessionId};
use temor_wire::link::Monotonic;
use temor_wire::message::{
    Event, Message, OpenSession, Palette, RemotePath, Reply, SessionInfo, SessionState,
};
use temor_wire::units::{CellPx as CellSize, Size};

use super::colours::apply_palette;
use super::config::DaemonConfig;
use super::exit::exit_status;
use super::input::{Backlog, Input};
use super::link::{Link, LinkId};
use super::sinks::{CwdSink, Replies};
use super::sizes::{check, pty_size, screen_cell, screen_size, usable};
use crate::clock::Clock;
use crate::error::Error;

/// How long a program may take to leave after its terminal hangs up, and how often that is polled.
const GRACE: Duration = Duration::from_millis(500);
const POLL: Duration = Duration::from_millis(20);

/// A session. `Send` and `Sync`: the pump thread and the connections share it.
#[derive(Debug)]
pub(super) struct Session {
    pub(super) id: SessionId,
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
    subscriber: Option<Arc<Link>>,
    detached_at: Option<Monotonic>,
    closing: Closing,
}

/// Whether a client asked to close the session. A hang-up alone can be lost (a program that has
/// not yet made its own process group misses it), so the pump also ends on this and reaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Closing {
    Open,
    Asked,
}

impl Session {
    /// Starts the program and its screen. The session begins detached. The terminal's read end is
    /// returned for the pump thread.
    pub(super) fn start(
        id: SessionId,
        request: &OpenSession,
        config: &DaemonConfig,
    ) -> Result<(Arc<Self>, PtyIo), Error> {
        let size = check(request.size)?;
        let cell = usable(request.cell);
        let program = request.shell.as_ref().map_or_else(
            || config.default_shell.clone(),
            |path| PathBuf::from(&path.0),
        );
        let mut env = ChildEnv::new();
        for var in &request.env {
            env.set(&var.name, &var.value);
        }
        let spec = SpawnSpec {
            program,
            args: Vec::new(),
            cwd: PathBuf::from(&request.cwd.0),
            env,
            size: pty_size(size, cell),
        };
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
                subscriber: None,
                detached_at: Some(config.clock.now()),
                closing: Closing::Open,
            }),
            pty: Mutex::new(pty),
            input,
            cwd,
            clock: Arc::clone(&config.clock),
        });
        Ok((session, reader))
    }

    /// Feeds program output to the screen and on to the attached client.
    pub(super) fn output(&self, bytes: &[u8]) {
        let mut shared = self.lock();
        shared.screen.write(bytes);
        let Some(link) = shared.subscriber.clone() else {
            return;
        };
        let event = Message::Event(Event::Output { session: self.id });
        if link.try_send(&event, Some(bytes)).is_err() {
            // The client is gone or too slow; it reattaches for a snapshot.
            shared.subscriber = None;
            shared.detached_at = Some(self.clock.now());
        }
    }

    pub(super) fn flush_input(&self) -> Backlog {
        self.input.flush()
    }

    pub(super) fn backlog(&self) -> Backlog {
        self.input.backlog()
    }

    pub(super) fn send_input(&self, bytes: &[u8]) {
        self.input.send(bytes);
    }

    /// Answers an attach request with the screen's snapshot, and from then on sends the live
    /// output to `link`. Both happen under the session's lock, so no output falls between them.
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
        shared.subscriber = Some(Arc::clone(link));
        shared.detached_at = None;
        Ok(())
    }

    /// Detaches the client, if it is `from` (or whoever it is when `from` is `None`).
    pub(super) fn detach(&self, from: Option<LinkId>) {
        let mut shared = self.lock();
        let matches = shared
            .subscriber
            .as_ref()
            .is_some_and(|link| from.is_none_or(|id| link.id == id));
        if matches {
            shared.subscriber = None;
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
        self.lock().closing = Closing::Asked;
        self.hang_up();
    }

    /// Whether a client asked to close the session.
    pub(super) fn close_asked(&self) -> bool {
        self.lock().closing == Closing::Asked
    }

    /// Hangs up the program, as closing a terminal window would. The pump thread reaps it.
    pub(super) fn hang_up(&self) {
        // Already gone is fine.
        let _ = self.pty().hang_up();
    }

    pub(super) fn info(&self) -> SessionInfo {
        let shared = self.lock();
        let cwd = self.cwd.lock().unwrap_or_else(|p| p.into_inner()).clone();
        SessionInfo {
            session: self.id,
            cwd: RemotePath(cwd),
            size: shared.size,
            state: if shared.subscriber.is_some() {
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
    pub(super) fn reap(&self) -> Option<ChildExit> {
        self.hang_up();
        let mut waited = Duration::ZERO;
        loop {
            {
                let mut pty = self.pty();
                if let Ok(Some(exit)) = pty.try_exit() {
                    return Some(exit);
                }
                if waited >= GRACE {
                    return pty.kill().ok();
                }
            }
            std::thread::sleep(POLL);
            waited += POLL;
        }
    }

    /// Tells the attached client the program ended.
    pub(super) fn exited(&self, exit: Option<ChildExit>) {
        let link = self.lock().subscriber.take();
        if let Some(link) = link {
            let event = Message::Event(Event::Exited {
                session: self.id,
                status: exit_status(exit),
            });
            // A client that cannot take it has gone; there is no one to tell.
            let _ = link.try_send(&event, None);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn pty(&self) -> MutexGuard<'_, Pty> {
        self.pty
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
