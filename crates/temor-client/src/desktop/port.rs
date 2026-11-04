//! The two halves of the window's link to the desktop services (PLAN §4b, §8.4).
//!
//! The bus runs on tokio worker threads; the window lives on its own thread. They meet here:
//! - [`DesktopPort`] (bus side) is what `temor-desktop` calls. Its reads (`context`, `panes`,
//!   `commands`) return from a [`Snapshot`] the window publishes after each draw. Its actions,
//!   and the reads of a pane's text, are sent to the window as an [`Ask`] and answered there,
//!   because only the window's thread may touch the panes.
//! - [`DesktopLink`] (window side) takes the asks, publishes the snapshot and the dock count.
//!
//! Both are `Send + Sync`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use temor_desktop::{
    BadgeCount, CommandInfo, CommandRef, LineRange, PaneInfo, PaneOpen, PaneRef, PortRefusal,
    Request, SandboxResult, SandboxRun, SpaceRef, WindowContext, WindowPort,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::watch;

/// How long the bus task waits for the window to answer before it says temor is busy.
pub const ASK_TIMEOUT: Duration = Duration::from_secs(5);

/// What the window shows the desktop services between draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub context: WindowContext,
    /// Every pane, in tab order.
    pub panes: Vec<PaneInfo>,
    /// Each pane's commands, oldest first.
    pub commands: BTreeMap<PaneRef, Vec<CommandInfo>>,
}

impl Snapshot {
    /// What the desktop sees before the window has drawn once.
    pub fn empty() -> Snapshot {
        Snapshot {
            context: WindowContext {
                title: String::new(),
                focused: None,
                last_command: None,
                selection: None,
                pane_count: 0,
                space: SpaceRef {
                    local: 0,
                    link: None,
                },
            },
            panes: Vec::new(),
            commands: BTreeMap::new(),
        }
    }
}

/// Something the window does for the desktop.
#[derive(Debug)]
pub enum Act {
    Open(PaneOpen),
    Close(PaneRef),
    Run(PaneRef, String),
    ReadPane(PaneRef, LineRange),
    ReadCommand(PaneRef, CommandRef),
}

/// What the window answers.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Pane(PaneRef),
    Text(String),
    Done,
}

/// Where an answer goes: a one-shot channel the bus task awaits.
pub type Reply = tokio::sync::oneshot::Sender<Result<Answer, PortRefusal>>;

/// A message for the window thread.
#[derive(Debug)]
pub enum Ask {
    /// An agent's action, answered through the reply.
    Act(Act, Reply),
    /// A later launch of temor asked for a window or a tab. The second field is the activation
    /// token it was started with, if any: the window raises itself with it.
    Launch(Request, Option<String>),
}

/// The bus side. Cheap to share.
#[derive(Debug, Clone)]
pub struct DesktopPort {
    snapshot: Arc<Mutex<Snapshot>>,
    asks: UnboundedSender<Ask>,
}

/// The window side.
#[derive(Debug, Clone)]
pub struct DesktopLink {
    snapshot: Arc<Mutex<Snapshot>>,
    asks: Arc<Mutex<Option<UnboundedReceiver<Ask>>>>,
    sender: UnboundedSender<Ask>,
    badge: Arc<watch::Sender<BadgeCount>>,
}

/// A port and its link, with the dock count's receiver for the bus task.
pub fn desktop_pair() -> (DesktopPort, DesktopLink, watch::Receiver<BadgeCount>) {
    let snapshot = Arc::new(Mutex::new(Snapshot::empty()));
    let (sender, receiver) = unbounded_channel();
    let (badge, count) = watch::channel(BadgeCount(0));
    let port = DesktopPort {
        snapshot: Arc::clone(&snapshot),
        asks: sender.clone(),
    };
    let link = DesktopLink {
        snapshot,
        asks: Arc::new(Mutex::new(Some(receiver))),
        sender,
        badge: Arc::new(badge),
    };
    (port, link, count)
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

impl DesktopLink {
    /// The asks, once: the window's loop takes the receiver and keeps it.
    pub fn take_asks(&self) -> Option<UnboundedReceiver<Ask>> {
        locked(&self.asks).take()
    }

    /// A sender of asks, for the bus task that hears later launches.
    pub fn asker(&self) -> UnboundedSender<Ask> {
        self.sender.clone()
    }

    /// Shows the desktop `snapshot` and the dock `count`. The dock hears only a changed count.
    pub fn publish(&self, snapshot: Snapshot, count: BadgeCount) {
        {
            let mut shown = locked(&self.snapshot);
            if *shown != snapshot {
                *shown = snapshot;
            }
        }
        self.badge.send_if_modified(|shown| {
            let changed = *shown != count;
            *shown = count;
            changed
        });
    }
}

impl DesktopPort {
    /// Hands `act` to the window and awaits its answer: busy when the window is gone or takes
    /// longer than [`ASK_TIMEOUT`]. The bus task's worker is free meanwhile.
    async fn ask(&self, act: Act) -> Result<Answer, PortRefusal> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.asks
            .send(Ask::Act(act, reply))
            .map_err(|_| PortRefusal::Busy)?;
        match tokio::time::timeout(ASK_TIMEOUT, answer).await {
            Ok(Ok(answer)) => answer,
            _ => Err(PortRefusal::Busy),
        }
    }

    async fn text(&self, act: Act) -> Result<String, PortRefusal> {
        match self.ask(act).await? {
            Answer::Text(text) => Ok(text),
            other => Err(PortRefusal::Failed(format!("unexpected answer: {other:?}"))),
        }
    }

    fn known(&self, pane: PaneRef) -> Result<Vec<CommandInfo>, PortRefusal> {
        let shown = locked(&self.snapshot);
        shown
            .commands
            .get(&pane)
            .cloned()
            .ok_or(PortRefusal::NoSuchPane)
    }
}

impl WindowPort for DesktopPort {
    fn context(&self) -> WindowContext {
        locked(&self.snapshot).context.clone()
    }

    fn panes(&self) -> Vec<PaneInfo> {
        locked(&self.snapshot).panes.clone()
    }

    async fn read_pane(&self, pane: PaneRef, range: LineRange) -> Result<String, PortRefusal> {
        self.text(Act::ReadPane(pane, range)).await
    }

    fn commands(&self, pane: PaneRef) -> Result<Vec<CommandInfo>, PortRefusal> {
        self.known(pane)
    }

    async fn read_command(
        &self,
        pane: PaneRef,
        command: CommandRef,
    ) -> Result<String, PortRefusal> {
        self.text(Act::ReadCommand(pane, command)).await
    }

    async fn open_pane(&self, open: PaneOpen) -> Result<PaneRef, PortRefusal> {
        match self.ask(Act::Open(open)).await? {
            Answer::Pane(pane) => Ok(pane),
            other => Err(PortRefusal::Failed(format!("unexpected answer: {other:?}"))),
        }
    }

    async fn close_pane(&self, pane: PaneRef) -> Result<(), PortRefusal> {
        self.ask(Act::Close(pane)).await.map(drop)
    }

    async fn run_in_pane(&self, pane: PaneRef, line: &str) -> Result<(), PortRefusal> {
        self.ask(Act::Run(pane, line.to_owned())).await.map(drop)
    }

    async fn run_sandboxed(&self, _run: SandboxRun) -> Result<SandboxResult, PortRefusal> {
        // The trial module that runs a command with no network arrives with P8.
        Err(PortRefusal::Failed(
            "running a command in a sandbox is not available yet".to_owned(),
        ))
    }

    fn undo(&self, _token: &str) -> Result<(), PortRefusal> {
        Err(PortRefusal::NoSuchCommand)
    }
}

#[cfg(test)]
mod tests;
