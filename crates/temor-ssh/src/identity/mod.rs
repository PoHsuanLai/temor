//! A host's identity (PLAN §9b gate, D18): its SSH host-key fingerprint plus the name the
//! person sees.

mod fingerprint;
#[cfg(test)]
mod tests;

pub use fingerprint::{Fingerprint, HostId, ShownName, fingerprint_from_keygen};
