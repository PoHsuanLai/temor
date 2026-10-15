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
//! - **Restore points.** [`CheckpointPort`] lists an agent run's restore points, says what a
//!   restore would change and performs it, through docket; [`SystemCheckpoints`] is the live one.
//! - **Tailscale.** [`SystemTailnet::hosts`] lists the person's other computers as
//!   [`temor_ssh::TailnetHost`]s, and [`SystemTailnet::changes`] says when to read again.
//!
//! The Linux services are behind the `quire-desktop` feature (PLAN D15). With the feature off, or
//! on another system, the same names exist and report the service as absent, so the window
//! needs no `cfg`. Nothing here spawns a thread; the caller owns the tokio runtime that zbus runs
//! on (docket and porter build zbus for tokio).
//!
//! Types that cross threads are `Send + Sync` unless their docs say otherwise.

mod badge;
mod checkpoints;
mod env;
mod error;
mod manifest;
mod platform;
mod request;
mod secrets;
mod tailnet;
mod window;

pub use badge::{Attention, Badge, BadgeCount, Progress};
pub use checkpoints::{
    CheckpointPort, NoCheckpoints, NotSavedWhy, PlanDigest, PointFault, PointId, PointList,
    PointPlan, PointRow, PointState, Rewind, RunState, SessionKey, TurnHow, TurnMark, TurnNo,
};
pub use env::{BusRoute, DesktopEnv};
pub use error::DesktopError;
pub use manifest::{
    APP_BUS_NAME, COMMAND_LIST, COMMAND_READ, COMMAND_RUN_IN_PANE, COMMAND_RUN_SANDBOXED,
    INSTANCE_BUS_NAME, MANIFEST, PANE_CWD, PANE_LIST, PANE_OPEN, PANE_READ,
};
pub use platform::{
    SystemCheckpoints, SystemDock, SystemInstance, SystemIntents, SystemLinks, SystemSecrets,
    SystemTailnet,
};
pub use request::{Claim, Handover, Primary, Request};
pub use secrets::{NoSecrets, SecretFault, SecretVault};
pub use tailnet::TailnetChanges;
pub use window::{
    CommandInfo, CommandRef, Exit, Focus, LineRange, Network, PaneInfo, PaneKind, PaneOpen,
    PaneRef, Placement, PortRefusal, SandboxResult, SandboxRun, SpaceRef, WindowContext,
    WindowPort,
};
