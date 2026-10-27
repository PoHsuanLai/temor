//! What the hub keeps about Tier B sessions (PLAN §9b): which hosts the person allowed the helper
//! on, each host's helper, connection and listening ports, and which panes are sessions on a
//! helper. The decisions are temor-core's `helper_step` and temor-wire's `link_step`; the files
//! beside this one carry them out.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use temor_core::remote::{Grant, HelperState, HostArch};
use temor_core::vocab::ids::PaneId;
use temor_pane::Feeder;
use temor_ssh::HostAlias;
use temor_wire::ids::SessionId;
use temor_wire::link::{LinkState, Monotonic};
use temor_wire::message::ListeningPort;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::edge::Ambient;
use temor_client::remote::builds::{HelperBuilds, arch_of};
use temor_client::remote::conn::Conn;
use temor_client::remote::grants;
use temor_client::remote::install::HostLogin;
use temor_panes::remote::RemoteWork;
use temor_panes::remote::event::{Generation, RemoteEvent};
use temor_panes::remote::link::Target;
use temor_panes::remote::route::Router;

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

/// The name the window gives this computer's own daemon among the hosts.
pub(super) const LOCAL: &str = "local";

/// How a pane that is a session in the local daemon starts, and what to do if the daemon cannot.
pub(super) struct LocalStart {
    pub spawn: temor_core::workspace::Spawn,
    pub cwd: String,
    pub env: Vec<(String, String)>,
    /// The program the person chose in settings; none is their login shell.
    pub shell: Option<String>,
    /// The session the pane had when temor last quit.
    pub resume: Option<SessionId>,
    /// The key the pane's shell signs its state reports with: the one it was started with when
    /// it goes on, else a new one (a session opened anew is given the same).
    pub report_key: temor_shell::report::ReportKey,
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
    /// The host's kind of processor, from the probe; none before it.
    pub arch: Option<HostArch>,
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
            arch: None,
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
    /// Panes that are sessions in this computer's own daemon, and how each starts.
    pub local: BTreeMap<PaneId, LocalStart>,
    /// A detach in progress: the pane giving up its program, and the session it takes along.
    pub detaching: Option<(PaneId, SessionId)>,
    /// The helper builds this temor carries, one per kind of processor.
    pub builds: HelperBuilds,
    started: Instant,
}

impl RemoteHub {
    pub(super) fn new(ambient: &Ambient, state_dir: Option<PathBuf>) -> Self {
        let (events, inbox) = unbounded_channel();
        let legacy = state_dir.as_deref().map(grants::load).unwrap_or_default();
        Self {
            work: RemoteWork::new(events),
            inbox: Some(inbox),
            legacy,
            lines: BTreeMap::new(),
            panes: BTreeMap::new(),
            saved: BTreeMap::new(),
            local: BTreeMap::new(),
            detaching: None,
            builds: HelperBuilds::new(
                arch_of(std::env::consts::ARCH),
                ambient.env.get("TEMOR_REMOTE_HELPER"),
                std::env::current_exe().ok().as_deref(),
            ),
            started: Instant::now(),
        }
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
