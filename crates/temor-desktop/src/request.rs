//! Single instance: what a later launch asks the first one to do, and the first one's side of
//! the claim.

use std::any::Any;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// What a second launch (or the desktop, through the bus) asks the running temor for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Open a new window.
    NewWindow,
    /// Open a new tab, in `cwd` when one is named.
    NewTab {
        /// The folder to start in.
        cwd: Option<PathBuf>,
    },
    /// Run a program in a new tab in place of the shell (`temor -e`).
    Run(RunRequest),
}

/// A program to run in a tab of its own, as the launch that asked for it would have run it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    /// The program and its arguments; never empty.
    pub argv: Vec<OsString>,
    /// The folder the asking launch was in.
    pub cwd: Option<PathBuf>,
    /// The asking launch's environment.
    pub env: Vec<(OsString, OsString)>,
    /// The asking launch's wait, when it waits for the tab to close: the window tells
    /// [`RunEnds`] this ticket once the tab is gone.
    pub ticket: Option<RunTicket>,
}

/// One `temor -e` launch's wait for its tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RunTicket(pub u64);

/// The status a waiting `temor -e` exits with when its tab closed while the program was still
/// running (or temor itself went away): a terminal's hang-up, as a shell reports it (128 + 1).
pub const CLOSED_WHILE_RUNNING: i32 = 129;

/// The status for a program that could not be found, as a shell reports it.
pub const NOT_FOUND: i32 = 127;

/// A run's tab is gone: the launch that waits on `ticket` exits with `status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunEnd {
    pub ticket: RunTicket,
    pub status: i32,
}

/// Where the window says a run's tab closed. Cheap to clone; the default tells no one (a window
/// without the bus, or a run nobody waits on).
#[derive(Debug, Clone, Default)]
pub struct RunEnds(Option<UnboundedSender<RunEnd>>);

impl RunEnds {
    /// Ends that go to `sender`.
    pub fn new(sender: UnboundedSender<RunEnd>) -> RunEnds {
        RunEnds(Some(sender))
    }

    /// The tab of `ticket` is gone, its program having ended with `status`.
    pub fn ended(&self, ticket: RunTicket, status: i32) {
        if let Some(sender) = &self.0 {
            let _ = sender.send(RunEnd { ticket, status });
        }
    }
}

/// A request and the activation token its sender was started with (a launcher, a notification
/// click), which lets the running window take the keyboard on Wayland. Empty means none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handover {
    /// What was asked.
    pub request: Request,
    /// The one-use activation token, when the sender had one.
    pub token: Option<String>,
}

/// The first launch's side: the requests other launches and agents send, until it is dropped.
/// Dropping it gives the name up.
pub struct Primary {
    requests: UnboundedReceiver<Handover>,
    run_ends: RunEnds,
    _guard: Box<dyn Any + Send + Sync>,
}

impl Primary {
    /// A primary that hears `requests` and keeps `guard` (the bus connection) alive.
    pub fn new(requests: UnboundedReceiver<Handover>, guard: Box<dyn Any + Send + Sync>) -> Self {
        Self {
            requests,
            run_ends: RunEnds::default(),
            _guard: guard,
        }
    }

    /// The same, telling `run_ends` where waiting launches hear that their tab closed.
    pub fn with_run_ends(self, run_ends: RunEnds) -> Self {
        Self { run_ends, ..self }
    }

    /// Where the window says a run's tab closed.
    pub fn run_ends(&self) -> RunEnds {
        self.run_ends.clone()
    }

    /// The next request, or `None` once nothing can send one.
    pub async fn next(&mut self) -> Option<Request> {
        self.next_handover().await.map(|heard| heard.request)
    }

    /// The next request with the activation token that came with it, or `None` once nothing can
    /// send one.
    pub async fn next_handover(&mut self) -> Option<Handover> {
        self.requests.recv().await
    }
}

impl fmt::Debug for Primary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Primary").finish_non_exhaustive()
    }
}

/// How a launch ended up.
#[derive(Debug)]
pub enum Claim {
    /// This launch is the first: it owns the name and receives the requests.
    Primary(Primary),
    /// Another temor is running and has been asked to do the request; this launch exits.
    Forwarded,
}
