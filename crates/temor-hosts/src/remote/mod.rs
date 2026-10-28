//! SSH hosts (§9b): the host vocabulary, the connection's machine and the helper's.

mod helper;
mod link;
mod model;
#[cfg(test)]
mod tests;

pub use helper::{
    CacheDir, HelperIn, HelperOut, HelperState, HostArch, HostOs, Probe, helper_step,
};
pub use link::{Link, LinkIn, LinkOut, LinkParams, link_step};
pub use model::{Grant, Host, HostConsent, RemoteSession, RemoteTier};
