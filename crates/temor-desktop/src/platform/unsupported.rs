//! Builds without the Linux desktop services: the same API, every service absent.
//! TODO(platform:macos): dock badge through NSDockTile, single instance through a socket.
//! TODO(platform:windows): taskbar overlay badge, single instance through a named pipe.

use crate::checkpoints::{
    CheckpointPort, PlanDigest, PointId, PointList, PointPlan, Rewind, SessionKey, TurnHow,
    TurnMark,
};
use crate::secrets::{SecretFault, SecretVault};
use crate::tailnet::TailnetChanges;
use crate::{Badge, Claim, DesktopEnv, DesktopError, Primary, Request, WindowPort};
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
#[derive(Debug)]
pub struct SystemDock;

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
            what: "restore points",
        })
    }
}

impl CheckpointPort for SystemCheckpoints {
    fn list(&self, _session: &SessionKey) -> Result<PointList, DesktopError> {
        Ok(PointList::default())
    }

    fn plan(&self, _session: &SessionKey, _id: PointId) -> Result<PointPlan, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "restore points",
        })
    }

    fn restore(
        &self,
        _session: &SessionKey,
        _id: PointId,
        _digest: &PlanDigest,
    ) -> Result<(), DesktopError> {
        Err(DesktopError::Unsupported {
            what: "restore points",
        })
    }

    fn watch(
        &self,
        _workspace: &str,
        _label: &str,
        _rewind: Rewind,
    ) -> Result<SessionKey, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "restore points",
        })
    }

    fn mark(&self, _session: &SessionKey) -> Result<TurnMark, DesktopError> {
        Err(DesktopError::Unsupported {
            what: "restore points",
        })
    }

    fn turn_ended(
        &self,
        _session: &SessionKey,
        _turn: TurnMark,
        _how: TurnHow,
    ) -> Result<(), DesktopError> {
        Ok(())
    }

    fn close(&self, _session: &SessionKey) -> Result<(), DesktopError> {
        Ok(())
    }
}

/// A Space's secrets. Absent here: nothing is kept and every ask finds nothing.
/// TODO(platform:macos): the Keychain through porter's keyring store.
/// TODO(platform:windows): the Credential Manager through the same.
#[derive(Debug)]
pub struct SystemSecrets;

impl SystemSecrets {
    /// Always [`SecretFault::NoStore`].
    pub fn new(_runtime: tokio::runtime::Handle) -> Result<Self, SecretFault> {
        Err(SecretFault::NoStore)
    }
}

impl SecretVault for SystemSecrets {
    fn get(&self, _account: &str) -> Result<Option<String>, SecretFault> {
        Ok(None)
    }

    fn put(&self, _account: &str, _secret: &str) -> Result<(), SecretFault> {
        Err(SecretFault::NoStore)
    }

    fn delete(&self, _account: &str) -> Result<(), SecretFault> {
        Err(SecretFault::NoStore)
    }
}
