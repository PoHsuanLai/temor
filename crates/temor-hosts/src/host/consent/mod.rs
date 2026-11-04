//! What the bootstrap may do on a host (§9b "Consent per host"): asked once, remembered per
//! machine, so a renamed host or a new alias is not asked again.

mod model;
mod step;
#[cfg(test)]
mod tests;

pub use model::{BootstrapAction, BootstrapGrants, ConsentBook, HostRecord, Permit, Who};
pub use step::{ConsentIn, Gate, RunMode, consent_step, gate};
