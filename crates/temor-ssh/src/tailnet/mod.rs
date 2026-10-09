//! A computer on the person's Tailscale network, as plain data.

mod host;
#[cfg(test)]
mod tests;

pub use host::{Reach, TailnetHost, TailnetNode, TailnetOwner, TailscaleSsh, UnixSeconds};
