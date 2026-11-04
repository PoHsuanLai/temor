//! The edge: everything that differs by system sits here. Linux with the `quire-desktop` feature
//! talks to the session bus; macOS with it has the Keychain and the Dock tile; every other
//! service, and every other build, gets the same names reporting the service absent.

#[cfg(all(target_os = "linux", feature = "quire-desktop"))]
mod linux;
#[cfg(all(target_os = "macos", feature = "quire-desktop"))]
mod macos;
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    feature = "quire-desktop"
))]
mod stored;
#[cfg(all(
    test,
    any(target_os = "linux", target_os = "macos"),
    feature = "quire-desktop"
))]
mod stored_tests;
#[cfg(not(all(target_os = "linux", feature = "quire-desktop")))]
mod unsupported;

#[cfg(all(target_os = "linux", feature = "quire-desktop"))]
pub use linux::{
    SystemDock, SystemInstance, SystemIntents, SystemLinks, SystemSecrets, SystemTailnet,
};
#[cfg(all(target_os = "macos", feature = "quire-desktop"))]
pub use macos::{SystemDock, SystemInstance, SystemSecrets};
#[cfg(not(all(
    any(target_os = "linux", target_os = "macos"),
    feature = "quire-desktop"
)))]
pub use unsupported::{
    SystemDock, SystemInstance, SystemIntents, SystemLinks, SystemSecrets, SystemTailnet,
};
#[cfg(all(target_os = "macos", feature = "quire-desktop"))]
pub use unsupported::{SystemIntents, SystemLinks, SystemTailnet};
