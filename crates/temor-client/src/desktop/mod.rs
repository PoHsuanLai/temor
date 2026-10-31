//! The window's side of the desktop (PLAN §4b, §8.4, D22): the port the desktop services call
//! and the window answers, and the Tailscale computers as the window reads them.

pub mod feed;
pub mod port;

// The port's vocabulary, which the window answers in: its requests and what they name.
pub use temor_desktop::{
    BadgeCount, CommandInfo, CommandRef, Exit, Focus, LineRange, PaneInfo, PaneKind, PaneOpen,
    PaneRef, Placement, PortRefusal, Request, SpaceRef, WindowContext,
};
// The desktop services the window starts and holds: the dock's badge, the single instance,
// the system's intents and password store, the Tailscale computers.
pub use temor_desktop::{
    Badge, BusRoute, Claim, DesktopEnv, DesktopError, NoSecrets, Primary, SecretVault, SystemDock,
    SystemInstance, SystemIntents, SystemLinks, SystemOpen, SystemSecrets, SystemTailnet,
};
// A program run for another (`temor run`): its ticket and how it ended.
pub use temor_desktop::{CLOSED_WHILE_RUNNING, NOT_FOUND, RunEnds, RunRequest, RunTicket};
