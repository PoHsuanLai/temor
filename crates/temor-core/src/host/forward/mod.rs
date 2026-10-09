//! Port forwards on a host's shared connection (§9b "Port forwarding"). `forward_step` tracks
//! the forwards of one host; the app runs the `ssh -O forward` and `-O cancel` commands it
//! names (temor-ssh's `forward_argv` and `cancel_forward_argv`). A tailnet machine's ports open
//! directly at `name:port` first (D22); `offers` lists the choices.

mod model;
mod offer;
mod step;
#[cfg(test)]
mod tests;

pub use model::{ForwardFault, ForwardIn, ForwardOut, Forwards};
pub use offer::{PortOffer, offers};
pub use step::forward_step;
