//! SSH hosts (§9b): the host vocabulary and the helper's machine. The connection's machine is
//! temor-wire's `link_step`.

mod helper;
mod model;
#[cfg(test)]
mod tests;

pub use helper::{
    CacheDir, HelperIn, HelperOut, HelperState, HostArch, HostOs, Probe, helper_step,
};
pub use model::Grant;
