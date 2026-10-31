//! The daemon itself: its sessions, its connections and the file service they share.

use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use temor_wire::hello::Capability;
use temor_wire::ids::RemoteSessionId;
use temor_wire::message::{IdleLimit, ListeningPort, Message, OpenSession};
use temor_wire::units::UnixMillis;

use super::config::DaemonConfig;
use super::link::Link;
use super::pump;
use super::registry::Registry;
use super::session::Session;
use crate::error::Error;
use crate::files::{FileService, STALE_AFTER, Scope};
use crate::platform;
use crate::sync::lock;

/// A session daemon. `Send` and `Sync`; share it as an `Arc`.
#[derive(Debug)]
pub struct Daemon {
    pub(super) config: DaemonConfig,
    pub(super) registry: Registry,
    pub(super) links: Mutex<Vec<Arc<Link>>>,
    pub(super) files: FileService,
    pub(super) idle: Mutex<IdleLimit>,
    pub(super) ports: Mutex<Vec<ListeningPort>>,
    pub(super) next_link: AtomicU64,
    /// Connections accepted and not yet ended, counted from the accept (before the greeting),
    /// so a daemon about to linger out does not miss one that is still saying hello.
    pub(super) accepted: Arc<AtomicUsize>,
}

/// One accepted connection, counted until it is dropped.
#[derive(Debug)]
pub struct Accepted(Arc<AtomicUsize>);

impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Daemon {
    /// A daemon with no sessions and no connections.
    pub fn new(config: DaemonConfig) -> Arc<Self> {
        Arc::new(Self {
            files: FileService::new(Scope::new(config.file_roots.clone())),
            idle: Mutex::new(config.idle),
            config,
            registry: Registry::default(),
            links: Mutex::new(Vec::new()),
            ports: Mutex::new(Vec::new()),
            next_link: AtomicU64::new(0),
            accepted: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Counts a connection the listener accepted, until the value is dropped (when serving it
    /// ends).
    pub fn accepted(&self) -> Accepted {
        self.accepted.fetch_add(1, Ordering::SeqCst);
        Accepted(Arc::clone(&self.accepted))
    }

    /// Starts a session and its pump thread.
    pub(super) fn open_session(
        self: &Arc<Self>,
        request: &OpenSession,
    ) -> Result<RemoteSessionId, Error> {
        let id = self.registry.next_id();
        let (session, reader) = Session::start(id, request, &self.config)?;
        // The session's folder is something the client asked for, so its files are reachable.
        // A folder that cannot be granted (not absolute) only leaves the files out of reach.
        let _ = self.files.grant(Path::new(&request.cwd.0));
        self.registry.insert(Arc::clone(&session));
        let daemon = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name(format!("temor-remote-session-{}", id.0))
            .spawn(move || pump::run(daemon, session, reader));
        if let Err(error) = spawned {
            self.remove_session(id);
            return Err(Error::io("starting a session's thread", error));
        }
        Ok(id)
    }

    /// Removes the staged parts of uploads that were never finished and are over a day old, in
    /// the folders the file service reaches now. The number removed. Run it once when the
    /// daemon starts, off the thread that accepts connections.
    pub fn sweep_staged(&self, now: UnixMillis) -> usize {
        self.files.sweep_staged(now, STALE_AFTER)
    }

    /// Hangs up every session. Their pump threads reap the programs. For a daemon being stopped
    /// on purpose; connections dropping never does this.
    pub fn shutdown(&self) {
        for session in self.registry.all() {
            session.hang_up();
        }
    }

    pub(super) fn remove_session(&self, id: RemoteSessionId) {
        self.registry.remove(id);
    }

    /// The listening ports of the sessions' process trees now. A host that cannot say reports
    /// none.
    pub(super) fn scan_ports(&self) -> Result<Vec<ListeningPort>, Error> {
        let pids: Vec<u32> = self.registry.all().iter().map(|s| s.pid()).collect();
        match platform::listening_ports(&self.config.proc_root, &pids) {
            Err(Error::Unsupported { .. }) => Ok(Vec::new()),
            other => other,
        }
    }

    /// Sends an event to every connection that agreed `needs`.
    pub(super) fn broadcast(&self, needs: Capability, message: &Message) {
        for link in self.links_snapshot() {
            if link.has(needs) {
                // A connection that cannot take it hangs itself up.
                let _ = link.try_send(message, None);
            }
        }
    }

    pub(super) fn links_snapshot(&self) -> Vec<Arc<Link>> {
        self.lock_links().clone()
    }

    pub(super) fn lock_links(&self) -> MutexGuard<'_, Vec<Arc<Link>>> {
        lock(&self.links)
    }
}
