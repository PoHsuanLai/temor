//! Builds without the Linux desktop services: the same API, every service absent.
//! TODO(platform:macos): single instance through a socket (the Keychain and the Dock are `macos.rs`).
//! TODO(platform:windows): taskbar overlay badge, single instance through a named pipe.

#[cfg(not(all(target_os = "macos", feature = "quire-desktop")))]
use crate::Badge;
use crate::checkpoints::{
    CheckpointPort, NoCheckpoints, PlanDigest, PointId, PointList, PointPlan, RESTORE_POINTS,
    SessionKey, TurnHow, TurnMark, WatchAsk, Watching,
};
#[cfg(not(all(target_os = "macos", feature = "quire-desktop")))]
use crate::secrets::{NoSecrets, SecretFault, SecretVault};
use crate::tailnet::TailnetChanges;
use crate::{Claim, DesktopEnv, DesktopError, Primary, Request, WindowPort};
use std::sync::Arc;
use temor_ssh::TailnetHost;
use tokio::sync::mpsc::unbounded_channel;

/// Single instance. Without a bus every launch is the first.
#[derive(Debug, Clone)]
pub struct SystemInstance;

impl SystemInstance {
    /// An instance claim in `env`.
    pub fn new(_env: DesktopEnv) -> Self {
        Self
    }

    /// This launch is the primary; nothing will ever send it a request.
    /// No bus: no running temor to hand a run to; this launch is the window itself.
    pub async fn run_and_wait(
        &self,
        _run: &crate::RunRequest,
        _token: Option<&str>,
    ) -> Result<Option<i32>, DesktopError> {
        Ok(None)
    }

    pub async fn claim(&self, _request: &Request) -> Result<Claim, DesktopError> {
        let (_sender, requests) = unbounded_channel();
        Ok(Claim::Primary(Primary::new(requests, Box::new(()))))
    }

    /// As [`claim`](Self::claim); there is no second launch to hand a token to.
    pub async fn claim_with(
        &self,
        request: &Request,
        _token: Option<&str>,
    ) -> Result<Claim, DesktopError> {
        self.claim(request).await
    }
}

/// temor's actions for agents. Absent here.
#[derive(Debug)]
pub struct SystemIntents;

impl SystemIntents {
    /// Always [`DesktopError::Unsupported`].
    pub async fn serve<W: WindowPort>(
        _env: &DesktopEnv,
        _window: Arc<W>,
    ) -> Result<Self, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "agent layer",
        })
    }

    /// Always [`DesktopError::Unsupported`].
    #[cfg(feature = "router-for-tests")]
    pub async fn serve_for_router<W: WindowPort>(
        _env: &DesktopEnv,
        _window: Arc<W>,
        _router: &str,
    ) -> Result<Self, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "agent layer",
        })
    }
}

/// The dock badge. Absent here.
#[cfg(not(all(target_os = "macos", feature = "quire-desktop")))]
#[derive(Debug)]
pub struct SystemDock;

#[cfg(not(all(target_os = "macos", feature = "quire-desktop")))]
impl SystemDock {
    /// Always [`DesktopError::Unsupported`].
    pub async fn connect(_env: &DesktopEnv) -> Result<Self, DesktopError> {
        Err(DesktopError::Unsupported { what: "dock" })
    }

    /// Never reached: there is no value of this type.
    pub async fn show(&self, _badge: &Badge) -> Result<(), DesktopError> {
        Err(DesktopError::Unsupported { what: "dock" })
    }
}

/// The desktop portal's link opener. Absent here.
#[derive(Debug)]
pub struct SystemLinks;

impl SystemLinks {
    /// Always [`DesktopError::Unsupported`].
    pub async fn connect(_env: &DesktopEnv) -> Result<Self, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "link opener",
        })
    }

    /// Never reached: there is no value of this type.
    pub async fn open(&self, _uri: &str) -> Result<(), DesktopError> {
        Err(DesktopError::Unsupported {
            what: "link opener",
        })
    }
}

/// The Tailscale computers. Absent here: the list is empty.
#[derive(Debug)]
pub struct SystemTailnet;

impl SystemTailnet {
    /// A reader that finds no computers.
    pub async fn connect(_env: &DesktopEnv) -> Result<Self, DesktopError> {
        Ok(Self)
    }

    /// Always empty.
    pub async fn hosts(&self) -> Result<Vec<TailnetHost>, DesktopError> {
        Ok(Vec::new())
    }

    /// A stream that has already ended.
    pub async fn changes(&self) -> Result<TailnetChanges, DesktopError> {
        Ok(TailnetChanges::ended())
    }
}

/// Docket's restore points. Absent here.
/// TODO(platform:macos): through the agent layer's transport once docket has one there.
/// TODO(platform:windows): the same.
#[derive(Debug)]
pub struct SystemCheckpoints;

impl SystemCheckpoints {
    /// Always [`DesktopError::Unsupported`].
    pub async fn connect(
        _env: &DesktopEnv,
        _runtime: tokio::runtime::Handle,
    ) -> Result<Self, DesktopError> {
        Err(DesktopError::Unsupported {
            what: RESTORE_POINTS,
        })
    }
}

/// Its constructor always fails, so no value of it exists; if one did, it would answer as
/// [`NoCheckpoints`] does.
impl CheckpointPort for SystemCheckpoints {
    fn list(&self, session: &SessionKey) -> Result<PointList, DesktopError> {
        NoCheckpoints.list(session)
    }

    fn plan(&self, session: &SessionKey, id: PointId) -> Result<PointPlan, DesktopError> {
        NoCheckpoints.plan(session, id)
    }

    fn restore(
        &self,
        session: &SessionKey,
        id: PointId,
        digest: &PlanDigest,
    ) -> Result<(), DesktopError> {
        NoCheckpoints.restore(session, id, digest)
    }

    fn watch(&self, ask: &WatchAsk) -> Result<Watching, DesktopError> {
        NoCheckpoints.watch(ask)
    }

    fn mark(&self, session: &SessionKey) -> Result<TurnMark, DesktopError> {
        NoCheckpoints.mark(session)
    }

    fn turn_ended(
        &self,
        session: &SessionKey,
        turn: TurnMark,
        how: TurnHow,
    ) -> Result<(), DesktopError> {
        NoCheckpoints.turn_ended(session, turn, how)
    }

    fn close(&self, session: &SessionKey) -> Result<(), DesktopError> {
        NoCheckpoints.close(session)
    }
}

/// A Space's secrets. Absent here: nothing is kept and every ask finds nothing.
/// TODO(platform:windows): the Credential Manager through the same.
#[cfg(not(all(target_os = "macos", feature = "quire-desktop")))]
#[derive(Debug)]
pub struct SystemSecrets;

#[cfg(not(all(target_os = "macos", feature = "quire-desktop")))]
impl SystemSecrets {
    /// Always [`SecretFault::NoStore`].
    pub fn new(_runtime: tokio::runtime::Handle) -> Result<Self, SecretFault> {
        Err(SecretFault::NoStore)
    }
}

/// Its constructor always fails, so no value of it exists; if one did, it would answer as
/// [`NoSecrets`] does.
#[cfg(not(all(target_os = "macos", feature = "quire-desktop")))]
impl SecretVault for SystemSecrets {
    fn get(&self, account: &str) -> Result<Option<String>, SecretFault> {
        NoSecrets.get(account)
    }

    fn put(&self, account: &str, secret: &str) -> Result<(), SecretFault> {
        NoSecrets.put(account, secret)
    }

    fn delete(&self, account: &str) -> Result<(), SecretFault> {
        NoSecrets.delete(account)
    }
}
