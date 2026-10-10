//! The person's other Tailscale computers, read from porter's account service
//! (`org.quire.Tailnet1`, PLAN §9b, D22). temor never talks to tailscaled. With no session bus,
//! no account service, no Tailscale account or a stopped Tailscale the list is empty.

use super::bus::session_builder;
use crate::tailnet::{TailnetChanges, Ticks};
use crate::{DesktopEnv, DesktopError};
use porter_client::{
    DbusTransport, Machine, MachineChanges, MachineLink, MachineOwner, MachineSsh, Tailnet,
    TailnetError,
};
use std::pin::Pin;
use std::task::{Context, Poll};
use temor_ssh::{NodeId, Presence, TailnetHost, TailnetOwner, TailscaleSsh, UnixSeconds};
use zbus::export::futures_core::Stream;

/// The tailnet list over the session bus.
#[derive(Debug)]
pub struct SystemTailnet {
    transport: Option<DbusTransport>,
}

impl SystemTailnet {
    /// A reader on the session bus of `env`; without a bus it finds no computers.
    pub async fn connect(env: &DesktopEnv) -> Result<Self, DesktopError> {
        let builder = match session_builder(env) {
            Ok(builder) => builder,
            Err(DesktopError::NoBus) => return Ok(Self { transport: None }),
            Err(error) => return Err(error),
        };
        let connection = builder
            .build()
            .await
            .map_err(|error| DesktopError::bus("connect to the session bus", error))?;
        Ok(Self {
            transport: Some(DbusTransport::over(connection)),
        })
    }

    /// The other computers on the person's tailnet, in the account service's order; this one is
    /// left out. Empty when Tailscale is absent.
    pub async fn hosts(&self) -> Result<Vec<TailnetHost>, DesktopError> {
        let Some(tailnet) = self.tailnet().await? else {
            return Ok(Vec::new());
        };
        match tailnet.machines().await {
            Ok(machines) => Ok(machines.iter().filter_map(host_of).collect()),
            Err(error) => absent_or_failed(error).map(|()| Vec::new()),
        }
    }

    /// A stream that ticks each time the list changed (read it again). Subscribe before the
    /// first [`SystemTailnet::hosts`] so no change is missed. Already ended when Tailscale is
    /// absent.
    pub async fn changes(&self) -> Result<TailnetChanges, DesktopError> {
        let Some(tailnet) = self.tailnet().await? else {
            return Ok(TailnetChanges::ended());
        };
        match tailnet.watch().await {
            Ok(changes) => Ok(TailnetChanges::over(Watch(changes))),
            Err(error) => absent_or_failed(error).map(|()| TailnetChanges::ended()),
        }
    }

    async fn tailnet(&self) -> Result<Option<Tailnet>, DesktopError> {
        let Some(transport) = &self.transport else {
            return Ok(None);
        };
        match transport.tailnet().await {
            Ok(tailnet) => Ok(Some(tailnet)),
            Err(error) => absent_or_failed(error).map(|()| None),
        }
    }
}

/// No account service is the same as no computers; anything else is a failure to report.
fn absent_or_failed(error: TailnetError) -> Result<(), DesktopError> {
    match error {
        TailnetError::Unreachable => Ok(()),
        other => Err(DesktopError::bus("read the Tailscale computers", other)),
    }
}

/// Porter's change stream as a source of ticks.
struct Watch(MachineChanges);

impl Ticks for Watch {
    fn poll_tick(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>> {
        Pin::new(&mut self.0).poll_next(cx)
    }
}

/// One machine as temor's plain host record; `None` for a machine whose node id temor can't use.
pub(super) fn host_of(machine: &Machine) -> Option<TailnetHost> {
    Some(TailnetHost {
        node: NodeId::new(machine.node.as_str())?,
        name: machine.name.clone(),
        dns: machine.dns.clone(),
        addresses: machine.addresses.clone(),
        online: match machine.link {
            MachineLink::Online => Presence::Online,
            MachineLink::Offline => Presence::Offline,
        },
        last_seen: machine
            .last_seen
            .and_then(|at| u64::try_from(at.0).ok())
            .map(UnixSeconds),
        ssh: match machine.ssh {
            MachineSsh::On => TailscaleSsh::On,
            MachineSsh::Off => TailscaleSsh::Off,
        },
        owner: match machine.owner {
            MachineOwner::Mine => TailnetOwner::Mine,
            MachineOwner::Shared => TailnetOwner::Shared,
            MachineOwner::Tagged => TailnetOwner::Tagged,
        },
    })
}
