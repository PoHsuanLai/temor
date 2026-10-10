//! What the hub keeps about Tier B sessions (PLAN §9b): which hosts the person allowed the helper
//! on, each host's helper, connection and listening ports, and which panes are sessions on a
//! helper. The decisions are temor-core's `helper_step` and temor-wire's `link_step`; the files
//! beside this one carry them out.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use temor_core::remote::{Grant, HelperState};
use temor_core::vocab::ids::PaneId;
use temor_pane::Feeder;
use temor_ssh::HostAlias;
use temor_wire::ids::SessionId;
use temor_wire::link::{LinkState, Monotonic};
use temor_wire::message::ListeningPort;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::edge::Ambient;
use crate::edge::remote::RemoteWork;
use crate::edge::remote_conn::Conn;
use crate::edge::remote_event::{Generation, RemoteEvent};
use crate::edge::remote_grants;
use crate::edge::remote_install::{HelperBlob, HostLogin, blob_path, read_blob};
use crate::edge::remote_link::Target;
use crate::edge::remote_route::Router;

/// What a host pane runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tier {
    /// A plain `ssh` in a terminal of this computer.
    Plain,
    /// A session on the host's helper.
    Helper,
}

/// What a connection is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Busy {
    Idle,
    Probing,
    Dialing,
}

/// Whether the helper has ever answered on this host in this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Reach {
    Never,
    Reached,
}

/// Where a pane's session stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shown {
    /// Waiting for the helper, or for the host to open the session.
    Opening,
    /// Open on the host and showing its screen.
    Attached,
    /// The host no longer has the session.
    Gone,
}

/// One pane that is a session on a helper.
pub(super) struct RemotePane {
    pub alias: HostAlias,
    pub target: Arc<Mutex<Target>>,
    pub feeder: Feeder,
    pub session: Option<SessionId>,
    pub shown: Shown,
}

/// One host's helper and connection.
pub(super) struct RemoteLine {
    pub helper: HelperState,
    pub busy: Busy,
    pub reach: Reach,
    pub link: LinkState,
    pub generation: Generation,
    pub conn: Option<Arc<Conn>>,
    pub router: Router,
    pub ports: Vec<ListeningPort>,
    /// The host's login variables, from the probe.
    pub login: HostLogin,
    /// Panes waiting for the connection to open their sessions.
    pub waiting: Vec<PaneId>,
}

impl Default for RemoteLine {
    fn default() -> Self {
        Self {
            helper: HelperState::Absent,
            busy: Busy::Idle,
            reach: Reach::Never,
            link: LinkState::Connected {
                sessions: Vec::new(),
            },
            generation: Generation::default(),
            conn: None,
            router: Router::default(),
            ports: Vec::new(),
            login: HostLogin::default(),
            waiting: Vec::new(),
        }
    }
}

/// The hub's Tier B state.
pub(super) struct RemoteHub {
    pub work: RemoteWork,
    pub inbox: Option<UnboundedReceiver<RemoteEvent>>,
    /// The answers the old `remote.json` held, read once at start and carried into the consent
    /// book by `Hub::remote_carry_over`; the file is never written again.
    pub legacy: BTreeMap<String, Grant>,
    pub lines: BTreeMap<HostAlias, RemoteLine>,
    pub panes: BTreeMap<PaneId, RemotePane>,
    /// The sessions panes were saved with, until each pane has its session again or is told the
    /// host lost it.
    pub saved: BTreeMap<PaneId, SessionId>,
    blob: OnceLock<Option<HelperBlob>>,
    blob_at: Option<PathBuf>,
    started: Instant,
}

impl RemoteHub {
    pub(super) fn new(ambient: &Ambient, state_dir: Option<PathBuf>) -> Self {
        let (events, inbox) = unbounded_channel();
        let legacy = state_dir
            .as_deref()
            .map(remote_grants::load)
            .unwrap_or_default();
        Self {
            work: RemoteWork::new(events),
            inbox: Some(inbox),
            legacy,
            lines: BTreeMap::new(),
            panes: BTreeMap::new(),
            saved: BTreeMap::new(),
            blob: OnceLock::new(),
            blob_at: blob_path(
                ambient.env.get("TEMOR_REMOTE_HELPER"),
                std::env::current_exe().ok().as_deref(),
            ),
            started: Instant::now(),
        }
    }

    /// The helper build this temor carries, read the first time it is asked for.
    pub(super) fn blob(&self) -> Option<&HelperBlob> {
        self.blob
            .get_or_init(|| self.blob_at.as_deref().and_then(read_blob))
            .as_ref()
    }

    /// Now, on the clock the connection's machine reads.
    pub(super) fn now(&self) -> Monotonic {
        Monotonic(u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX))
    }

    /// The connection to `host`'s helper, when it is up now.
    pub(super) fn helper_conn(&self, host: &HostAlias) -> Option<Arc<Conn>> {
        let line = self.lines.get(host)?;
        match line.link {
            LinkState::Connected { .. } => line.conn.clone(),
            _ => None,
        }
    }

}
