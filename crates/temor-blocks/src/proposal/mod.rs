//! Proposals in your prompt (§8.3): sanitising, the trial verdict and the machine.

mod as_run;
mod model;
mod sanitise;
#[cfg(test)]
mod sanitise_tests;
mod step;
#[cfg(test)]
mod tests;
mod ucd_tables;

pub use model::{
    AskNo, ChangeKind, Cleaned, FileChange, FileChanges, Proposal, ProposalPhase, ProposalState,
    TrialAttempt, TrialId, TrialVerdict, UncheckedWhy,
};
pub use sanitise::{CharClass, Sanitised, SanitisedLine, sanitise};
pub use step::{AsRun, ProposalIn, ProposalOut, proposal_key, proposal_step};
pub use ucd_tables::UCD_VERSION;
