//! The handshake: each side says what it speaks and what it can do, and both compute the same
//! agreement from the two greetings.

mod model;
mod negotiate;

#[cfg(test)]
mod tests;

pub use model::{
    Capability, HELPER_CACHE_FOLDER, Hello, HelperVersion, ProtocolRange, ProtocolVersion,
};
pub use negotiate::{Agreement, Incompatibility, Negotiation, negotiate};
