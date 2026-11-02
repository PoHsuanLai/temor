//! What the hub keeps about SSH (PLAN §9b): the remembered answers, each host's shared connection
//! and forwards, which panes are sessions on a host, and the remote files being edited. The
//! steps that decide are temor-core's (`host::*`); the files beside this one carry them out.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use temor_client::model::host::consent::{ConsentBook, RunMode};
use temor_client::model::host::edit::{Edit, Watch};
use temor_client::model::host::forward::Forwards;
use temor_client::model::host::master::Master;
use temor_client::model::host::store::{HostsFile, PaneHost};
use temor_client::model::vocab::ids::PaneId;
use temor_client::ssh::{CheckUrl, DirectHost, HostAlias, HostKey, RemoteFilePath};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use temor_client::remote::message::FileStamp;

use temor_client::desktop::feed::{NoTailnet, TailnetFeed};
use temor_client::remote::edit_way::EditTier;
use temor_client::ssh::SshEnv;
use temor_client::ssh::work::{EditId, SshEvent, SshWork};
use temor_pane_view::platform::{FileShell, Opener, default_file_shell, default_opener};
use temor_ui_base::edge::Ambient;

/// Where a host pane's session stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    /// The person is being asked what the setup may do.
    Asking(temor_client::model::host::consent::BootstrapAction),
    /// Answered; waiting for the shared connection's turn.
    Waiting,
    /// The session is starting and the connection is not up yet (a password, a browser check).
    Connecting,
    /// Connected.
    Up,
    /// The session ended before it was up, or the connection could not be placed.
    Failed,
    /// The tailnet says the computer is offline: the person is told, and may try anyway.
    Offline,
}

/// The browser check the host asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Check {
    Nothing,
    /// The link is shown with an Open button; nothing has opened it.
    Shown(CheckUrl),
}

/// One pane that is a session on a host.
pub(super) struct HostPane {
    pub alias: HostAlias,
    pub phase: Phase,
    /// How the session starts, once every question is answered.
    pub mode: Option<RunMode>,
    /// The folder on the host the session opens in.
    pub start_dir: Option<RemoteFilePath>,
    /// Looks for the check link in what the host printed, until the shell starts.
    pub scan: Option<Arc<super::ssh_check::RawCheck>>,
    pub check: Check,
    /// What the pane's session runs on.
    pub tier: super::remote::Tier,
    /// A restored pane whose connection waits until its tab is first in front, so a launch does
    /// not ask for a password in every tab the last session had open.
    pub deferred: bool,
}

/// One host's shared connection, forwards and identity.
#[derive(Default)]
pub(super) struct HostLine {
    pub master: Master,
    pub forwards: Forwards,
    /// What is known of the machine behind the alias: its node id before any connection, its
    /// fingerprint once one verified it.
    pub key: Option<HostKey>,
    /// Where the host's ports can be opened without a forward, when it is a tailnet machine that
    /// is online.
    pub direct: Option<DirectHost>,
}

/// One remote file open for editing.
pub(super) struct RemoteEdit {
    pub host: HostAlias,
    pub path: RemoteFilePath,
    pub local: std::path::PathBuf,
    pub state: Edit,
    pub watch: Option<Watch>,
    /// How the file is fetched and written back.
    pub tier: EditTier,
    /// The host's file as the helper last read or wrote it; the write-back must still find it.
    /// Always `None` for a host without a helper.
    pub seen: Option<FileStamp>,
}

/// The hub's SSH state.
pub(super) struct SshHub {
    pub env: SshEnv,
    pub work: SshWork,
    pub inbox: Option<UnboundedReceiver<SshEvent>>,
    pub book: ConsentBook,
    pub tailnet: Arc<dyn TailnetFeed>,
    pub lines: BTreeMap<HostAlias, HostLine>,
    /// Panes of the last session that were on hosts; they reconnect as their panes start.
    pub restored: Vec<PaneHost>,
    pub edits: BTreeMap<EditId, RemoteEdit>,
    pub next_edit: u64,
    /// Files asked for on a host that was not connected yet; they open once it is.
    pub waiting: Vec<super::ssh_wait::WaitingEdit>,
    pub opener: Box<dyn Opener>,
    pub file_opener: Box<dyn FileShell>,
    /// The finished sockets in the connection folder when each connection being made started:
    /// a new one means that connection is up.
    pub baselines: BTreeMap<HostAlias, BTreeSet<String>>,
    /// Where the last tick polled the connections, to space the probes.
    pub ticks: u32,
}

impl SshHub {
    pub(super) fn new(ambient: &Ambient) -> Self {
        Self::with_env(ambient.ssh_env())
    }

    pub(super) fn with_env(env: SshEnv) -> Self {
        let (events, inbox): (UnboundedSender<SshEvent>, _) = unbounded_channel();
        Self {
            work: SshWork::new(env.clone(), events),
            env,
            inbox: Some(inbox),
            book: ConsentBook::default(),
            tailnet: Arc::new(NoTailnet),
            lines: BTreeMap::new(),
            restored: Vec::new(),
            edits: BTreeMap::new(),
            next_edit: 1,
            waiting: Vec::new(),
            opener: default_opener(),
            file_opener: default_file_shell(),
            baselines: BTreeMap::new(),
            ticks: 0,
        }
    }

    /// What the session file keeps: the answers, and which panes are on which host.
    pub(super) fn hosts_file(&self, panes: &super::panes::Panes) -> HostsFile {
        HostsFile {
            book: self.book.clone(),
            panes: panes
                .hosts()
                .map(|(pane, host)| PaneHost {
                    pane,
                    alias: host.alias.as_str().to_owned(),
                    // The hub adds the helper session (`Hub::hosts_file`).
                    session: None,
                })
                .collect(),
        }
    }
}

impl super::model::Hub {
    /// Replaces how links and files are handed to the desktop (tests record them).
    pub fn set_openers(&mut self, links: Box<dyn Opener>, files: Box<dyn FileShell>) {
        self.ssh.opener = links;
        self.ssh.file_opener = files;
    }

    /// The host `pane` is a session on, if it is one.
    pub fn host_of(&self, pane: PaneId) -> Option<&str> {
        self.panes.host(pane).map(|host| host.alias.as_str())
    }
}
