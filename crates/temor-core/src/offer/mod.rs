//! The shell offer (PLAN P17): a typed `ssh`, `docker exec`, `sudo -i` and the like gets one
//! quiet offer to load temor's shell integration in the inner shell, so its commands become
//! cards. Matching reads only the first words of the command (never output); the timing is a
//! small machine. Never automatic: nothing is written to the pane before a Yes.

mod list;
mod secret;
mod step;
#[cfg(test)]
mod tests;

pub use list::{DEFAULT_COMMANDS, OfferList, Program};
pub use secret::asks_for_secret;
pub use step::{
    Answer, Line, OfferIn, OfferOut, OfferState, PROBE_MS, QUIET_MS, Reply, SETTLE_MS, Screen,
    offer_step,
};
