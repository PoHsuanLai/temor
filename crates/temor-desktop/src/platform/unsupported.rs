//! Builds without the Linux desktop services: the same API, every service absent.
//! TODO(platform:macos): dock badge through NSDockTile, single instance through a socket.
//! TODO(platform:windows): taskbar overlay badge, single instance through a named pipe.

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
