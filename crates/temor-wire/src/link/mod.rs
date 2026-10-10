//! The reconnect machine (PLAN §9b): when the link to a host drops, back off, dial again, and
//! reattach every session of that host.
//!
//! Pure: events in, effects out. The caller supplies the clock (every event carries `at`), runs
//! the effects (dial, attach, a timer for `WakeAt`) and feeds the outcomes back as events.

mod model;
mod policy;
mod step;

#[cfg(test)]
mod tests;

pub use model::{GoneReason, LinkEffect, LinkEvent, LinkState, Millis, Monotonic};
pub use policy::{Attempt, PingPolicy, RetryPolicy};
pub use step::link_step;
