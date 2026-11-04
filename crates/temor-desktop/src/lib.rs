//! temor's desktop integration (PLAN §2, §8.4, §9b, §11 P6). This is the one crate that names
//! zbus, docket or porter-client; the window calls the small API below and never touches them.
//!
//! - **Actions for agents.** [`WindowPort`] is what the window implements (its panes, commands and
//!   context); [`SystemIntents`] serves temor's actions from `dist/intents/org.quire.Temor.toml`
//!   to the desktop's agent layer and answers `context()` from [`WindowPort::context`].
//! - **One instance.** [`SystemInstance::claim`] makes the first launch the owner of the bus name
//!   and hands a later launch's [`Request`] (a new window or tab) to it. The files in `dist/` let
//!   the bus start temor when a call arrives and none runs.
//! - **Dock badge.** [`SystemDock::show`] publishes a count, progress and attention.
//! - **Links.** [`SystemLinks::open`] hands a link to the desktop portal's opener.
//! - **Opening.** [`SystemOpen`] hands a file, a folder or a link to the desktop's own opener.
//! - **Tailscale.** [`SystemTailnet::hosts`] lists the person's other computers as
//!   [`temor_ssh::TailnetHost`]s, and [`SystemTailnet::changes`] says when to read again.
//!
//! The Linux services are behind the `quire-desktop` feature (PLAN D15). With the feature off, or
//! on another system, the same names exist and report the service as absent, so the window
//! needs no `cfg`. The caller owns the tokio runtime that zbus runs on (docket and porter build
//! zbus for tokio); the one thread started here waits for an opener ([`SystemOpen`]).
//!
//! Types that cross threads are `Send + Sync` unless their docs say otherwise.

mod badge;
mod env;
mod error;
mod manifest;
mod open;
mod platform;
mod request;
mod secrets;
mod tailnet;
mod window;

pub use badge::{Badge, BadgeCount, Progress};
pub use env::{BusRoute, DesktopEnv};
pub use error::DesktopError;
pub use manifest::{
    APP_BUS_NAME, COMMAND_LIST, COMMAND_READ, COMMAND_RUN_IN_PANE, COMMAND_RUN_SANDBOXED,
    INSTANCE_BUS_NAME, MANIFEST, PANE_CWD, PANE_LIST, PANE_OPEN, PANE_READ,
};
pub use open::SystemOpen;
pub use platform::{
    SystemDock, SystemInstance, SystemIntents, SystemLinks, SystemSecrets, SystemTailnet,
};
pub use request::{
    CLOSED_WHILE_RUNNING, Claim, Handover, NOT_FOUND, Primary, Request, RunEnd, RunEnds,
    RunRequest, RunTicket,
};
pub use secrets::{NoSecrets, SecretFault, SecretVault};
pub use tailnet::TailnetChanges;
pub use window::{
    CommandInfo, CommandRef, Exit, Focus, LineRange, PaneInfo, PaneKind, PaneOpen, PaneRef,
    Placement, PortRefusal, SandboxResult, SandboxRun, SpaceRef, WindowContext, WindowPort,
};
