//! One session: a program on a terminal, and the screen state libghostty keeps for it. Its pump
//! thread owns all of it ([`Pump`]); everyone else holds a [`Session`] and sends it
//! [`Command`]s.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

use temor_pty::{Pty, Wake, Waker};
use temor_vt::{Terminal, TerminalConfig};
use temor_wire::ids::{RemoteSessionId, RequestId};
use temor_wire::message::{
    IdleLimit, Message, OpenSession, Palette, RemoteError, Reply, SessionInfo,
};
use temor_wire::units::{CellPx as CellSize, Size};

use super::colours::apply_palette;
use super::config::DaemonConfig;
use super::input::Input;
use super::launch::spawn_spec;
use super::link::{Link, LinkId};
use super::pump::Pump;
use super::session_core::SessionCore;
use super::sinks::{CwdSink, Replies};
use super::sizes::{check, pty_size, screen_cell, screen_size, usable};
use crate::error::Error;

/// What a session is sent. Its pump takes them in order.
pub(super) enum Command {
    /// Answer with the screen's snapshot, then send the live output too.
    Attach(Answer),
    Detach(LinkId),
    /// The person's input.
    Input(Vec<u8>),
    /// The screen's answer to a query the program made.
    Reply(Vec<u8>),
    /// The folder the program said it is in.
    Cwd(String),
    Resize {
        size: Size,
        cell: CellSize,
        answer: Answer,
    },
    Palette {
        palette: Palette,
        answer: Answer,
    },
    /// Close it for a client: the program is hung up and reaped, even if it missed the hang-up.
    Close,
    /// Hang the program up, as closing a terminal window would.
    HangUp,
    Idle(IdleLimit),
    /// Time passed: a session detached too long ends.
    Tick,
    Info(Sender<SessionInfo>),
}

/// A request a session answers itself. One dropped unanswered (the session ended first) answers
/// that there is no such session.
pub(super) struct Answer {
    to: Option<(Arc<Link>, RequestId)>,
    session: RemoteSessionId,
}

impl Answer {
    pub(super) fn new(link: &Arc<Link>, request: RequestId, session: RemoteSessionId) -> Self {
        Self {
            to: Some((Arc::clone(link), request)),
            session,
        }
    }

    pub(super) fn link(&self) -> Option<&Arc<Link>> {
        self.to.as_ref().map(|(link, _)| link)
    }

    pub(super) fn give(
        mut self,
        outcome: Result<Reply, RemoteError>,
        payload: Option<&[u8]>,
    ) -> Result<(), Error> {
        match self.to.take() {
            Some((link, id)) => link.send(&Message::Response { id, outcome }, payload),
            None => Ok(()),
        }
    }
}

impl Drop for Answer {
    fn drop(&mut self) {
        if let Some((link, id)) = self.to.take() {
            let session = self.session;
            let outcome = Err(RemoteError::NoSuchSession { session });
            // A client that has gone cannot be told.
            let _ = link.send(&Message::Response { id, outcome }, None);
        }
    }
}

/// A session as the daemon holds it. `Send` and `Sync`.
#[derive(Debug)]
pub(super) struct Session {
    pub(super) id: RemoteSessionId,
    pid: u32,
    commands: Sender<Command>,
    waker: Waker,
}

impl Session {
    /// Starts the program and its screen, and the pump that is to own them. The session begins
    /// detached.
    pub(super) fn start(
        id: RemoteSessionId,
        request: &OpenSession,
        config: &DaemonConfig,
    ) -> Result<(Self, Pump), Error> {
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
        let io = pty.io()?;
        let input = Input::new(pty.io()?);
        let (commands, inbox) = channel();
        let mut screen_config = TerminalConfig::new(screen_size(size));
        screen_config.cell_px = screen_cell(cell);
        let mut screen = Terminal::new(
            screen_config,
            Box::new(CwdSink(commands.clone())),
            Box::new(Replies(commands.clone())),
        )?;
        if let Some(palette) = &request.palette {
            apply_palette(&mut screen, palette)?;
        }
        let wake = Wake::new()?;
        let session = Self {
            id,
            pid: u32::try_from(pty.pid().0).unwrap_or(0),
            commands,
            waker: wake.waker(),
        };
        let core = SessionCore::new(size, request.cwd.0.clone(), config.idle, config.clock.now());
        let pump = Pump {
            id,
            core,
            screen,
            pty,
            io,
            input,
            inbox,
            wake,
            clock: Arc::clone(&config.clock),
        };
        Ok((session, pump))
    }

    /// Sends `command` to the session. One sent after the session ended is dropped (an
    /// [`Answer`] in it says so).
    pub(super) fn send(&self, command: Command) {
        if self.commands.send(command).is_ok() {
            self.waker.wake();
        }
    }

    /// Where it is and who is attached; `None` once it has ended.
    pub(super) fn info(&self) -> Option<SessionInfo> {
        let (tx, rx) = channel();
        self.send(Command::Info(tx));
        rx.recv().ok()
    }

    /// The program's process id.
    pub(super) fn pid(&self) -> u32 {
        self.pid
    }
}
