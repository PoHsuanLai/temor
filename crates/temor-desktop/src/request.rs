//! Single instance: what a later launch asks the first one to do, and the first one's side of
//! the claim.

use std::any::Any;
use std::fmt;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedReceiver;

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
}

/// The first launch's side: the requests other launches and agents send, until it is dropped.
/// Dropping it gives the name up.
pub struct Primary {
    requests: UnboundedReceiver<Request>,
    _guard: Box<dyn Any + Send + Sync>,
}

impl Primary {
    /// A primary that hears `requests` and keeps `guard` (the bus connection) alive.
    pub fn new(requests: UnboundedReceiver<Request>, guard: Box<dyn Any + Send + Sync>) -> Self {
        Self {
            requests,
            _guard: guard,
        }
    }

    /// The next request, or `None` once nothing can send one.
    pub async fn next(&mut self) -> Option<Request> {
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
