//! The edge: everything that differs by system sits here. Linux with the `quire-desktop` feature
//! talks to the session bus; every other build gets the same names reporting the service absent.

#[cfg(all(target_os = "linux", feature = "quire-desktop"))]
mod linux;
#[cfg(not(all(target_os = "linux", feature = "quire-desktop")))]
mod unsupported;

#[cfg(all(target_os = "linux", feature = "quire-desktop"))]
pub use linux::{
    SystemCheckpoints, SystemDock, SystemInstance, SystemIntents, SystemSecrets, SystemTailnet,
};
#[cfg(not(all(target_os = "linux", feature = "quire-desktop")))]
pub use unsupported::{
    SystemCheckpoints, SystemDock, SystemInstance, SystemIntents, SystemSecrets, SystemTailnet,
};
