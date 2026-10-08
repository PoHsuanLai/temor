//! Proposals in your prompt (§8.3): sanitising, the trial verdict and the machine.

mod model;
mod sanitise;
#[cfg(test)]
mod sanitise_tests;
mod step;
#[cfg(test)]
mod tests;

pub use model::{ChangeKind, FileChange, FileChanges, Proposal, TrialVerdict, UncheckedWhy};
pub use sanitise::{CharClass, Sanitised, SanitisedLine, sanitise};
pub use step::{
    AsRun, ProposalIn, ProposalOut, ProposalState, TrialAttempt, proposal_key, proposal_step,
};
