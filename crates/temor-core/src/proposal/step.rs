//! `proposal_step`: from the person asking, through the model and the trial, to the line typed
//! into their prompt, where their own Enter is the approval (§8.3).
//!
//! Asked for, never offered: only `Ask` starts it. A failed trial goes back to the model once;
//! a second failure is shown with an offer to ask an agent. Escape and Ctrl-C go through
//! [`proposal_key`], which always hands the key on to the shell.

use super::model::{Proposal, TrialVerdict};
use super::sanitise::{Sanitised, SanitisedLine, sanitise};
use crate::block::Taint;
use crate::key::{PassThrough, ShellKey};
use crate::vocab::ai::AiSwitch;
use crate::vocab::ids::TurnRef;
use crate::vocab::text::{CommandText, ErrorLine};
use ds_core::word::Word;

/// Whether the model has already seen one failed trial for this ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TrialAttempt {
    /// The first answer.
    First,
    /// The answer after a failed trial; a second failure is shown as it is.
    AfterFailure,
}

/// Where a proposal stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalState {
    /// Nothing asked.
    Off,
    /// Waiting for the model.
    Asking {
        /// The ask.
        turn: TurnRef,
        /// Which answer this is.
        attempt: TrialAttempt,
    },
    /// The answer is being trial-run.
    Trialling {
        /// The cleaned answer.
        line: SanitisedLine,
        /// What cleaning removed.
        sanitised: Sanitised,
        /// The ask.
        turn: TurnRef,
        /// Whether the turn read untrusted text.
        taint: Taint,
        /// Which answer this is.
        attempt: TrialAttempt,
    },
    /// The chip shows the proposal and its verdict.
    Shown {
        /// The proposal.
        proposal: Proposal,
        /// What cleaning removed.
        sanitised: Sanitised,
    },
    /// The proposal is typed into the prompt, waiting for the person's Enter.
    Typed {
        /// The proposal.
        proposal: Proposal,
        /// What cleaning removed.
        sanitised: Sanitised,
    },
}

/// What moves a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalIn {
    /// The person asked (the Fix chip, or the bar's "Suggest a command" row).
    Ask(TurnRef),
    /// The model answered an ask.
    ModelAnswer {
        /// Which ask.
        turn: TurnRef,
        /// Its raw text.
        text: String,
        /// Whether the turn read untrusted text.
        taint: Taint,
    },
    /// The model could not answer.
    ModelFailed(TurnRef),
    /// The trial run finished.
    TrialResult(TrialVerdict),
    /// The person took the proposal (a click on the chip, or ⌘↩, which the shell never gets).
    Accept,
    /// The shell left the prompt to run `command` (OSC 133 C).
    LeftPrompt(CommandText),
}

/// Whether the command that ran was the proposal as typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum AsRun {
    /// Exactly the proposal.
    AsProposed,
    /// The person changed it first.
    Edited,
}

/// What a proposal wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalOut {
    /// Ask the model; after a failed trial, with the error it printed.
    CallModel {
        /// The ask.
        turn: TurnRef,
        /// The failed trial's first error, shown to the model once.
        failed: Option<ErrorLine>,
    },
    /// Trial-run this line in the sandbox.
    RunTrial(SanitisedLine),
    /// Show the chip.
    Show(Proposal),
    /// The proposal failed twice: offer "Ask an agent".
    OfferAgent,
    /// Type this line into the prompt by bracketed paste.
    TypeIntoPrompt(SanitisedLine),
    /// The model had nothing usable to propose.
    NothingToPropose,
    /// The chip went away without the proposal running.
    Dismissed,
    /// The person ran it: log it as their act.
    Ran {
        /// What was proposed.
        proposal: Proposal,
        /// What cleaning removed.
        sanitised: Sanitised,
        /// Whether they changed it.
        as_run: AsRun,
    },
}

/// The proposal after `input`. Answers to an ask that is no longer current are ignored. While
/// `ai` is off nothing reaches a model: any step drops what is under way.
pub fn proposal_step(
    state: ProposalState,
    input: ProposalIn,
    ai: AiSwitch,
) -> (ProposalState, Vec<ProposalOut>) {
    use ProposalState as S;
    if ai == AiSwitch::Off {
        let out = match state {
            S::Off => vec![],
            _ => vec![ProposalOut::Dismissed],
        };
        return (S::Off, out);
    }
    match (state, input) {
        (_, ProposalIn::Ask(turn)) => (
            S::Asking {
                turn,
                attempt: TrialAttempt::First,
            },
            vec![ProposalOut::CallModel { turn, failed: None }],
        ),
        (S::Off, _) => (S::Off, vec![]),
        (
            S::Asking { turn, attempt },
            ProposalIn::ModelAnswer {
                turn: answered,
                text,
                taint,
            },
        ) if answered == turn => {
            let (line, sanitised) = sanitise(&text);
            if line.as_str().trim().is_empty() {
                return (S::Off, vec![ProposalOut::NothingToPropose]);
            }
            let out = vec![ProposalOut::RunTrial(line.clone())];
            (
                S::Trialling {
                    line,
                    sanitised,
                    turn,
                    taint,
                    attempt,
                },
                out,
            )
        }
        (S::Asking { turn, .. }, ProposalIn::ModelFailed(failed)) if failed == turn => {
            (S::Off, vec![ProposalOut::NothingToPropose])
        }
        (
            S::Trialling {
                line,
                sanitised,
                turn,
                taint,
                attempt,
            },
            ProposalIn::TrialResult(verdict),
        ) => trialled(line, sanitised, turn, taint, attempt, verdict),
        (
            S::Shown {
                proposal,
                sanitised,
            },
            ProposalIn::Accept,
        ) => {
            let out = vec![ProposalOut::TypeIntoPrompt(proposal.text.clone())];
            (
                S::Typed {
                    proposal,
                    sanitised,
                },
                out,
            )
        }
        (
            S::Typed {
                proposal,
                sanitised,
            },
            ProposalIn::LeftPrompt(command),
        ) => {
            let as_run = if command.as_str() == proposal.text.as_str() {
                AsRun::AsProposed
            } else {
                AsRun::Edited
            };
            (
                S::Off,
                vec![ProposalOut::Ran {
                    proposal,
                    sanitised,
                    as_run,
                }],
            )
        }
        (_, ProposalIn::LeftPrompt(_)) => (S::Off, vec![ProposalOut::Dismissed]),
        (state, _) => (state, vec![]),
    }
}

/// Escape or Ctrl-C while a proposal is up: the chip goes, and the key goes on to the shell
/// whatever the state, so Ctrl-C still clears the line and Escape still means what the shell
/// makes it mean.
pub fn proposal_key(
    state: ProposalState,
    key: ShellKey,
) -> (ProposalState, Vec<ProposalOut>, PassThrough) {
    let out = match state {
        ProposalState::Off => vec![],
        _ => vec![ProposalOut::Dismissed],
    };
    (ProposalState::Off, out, PassThrough(key))
}

fn trialled(
    line: SanitisedLine,
    sanitised: Sanitised,
    turn: TurnRef,
    taint: Taint,
    attempt: TrialAttempt,
    verdict: TrialVerdict,
) -> (ProposalState, Vec<ProposalOut>) {
    if let (TrialVerdict::Failed { first_error, .. }, TrialAttempt::First) = (&verdict, attempt) {
        let out = vec![ProposalOut::CallModel {
            turn,
            failed: Some(first_error.clone()),
        }];
        let state = ProposalState::Asking {
            turn,
            attempt: TrialAttempt::AfterFailure,
        };
        return (state, out);
    }
    let failed = matches!(verdict, TrialVerdict::Failed { .. });
    let proposal = Proposal {
        text: line,
        turn,
        taint,
        verdict,
    };
    let mut out = vec![ProposalOut::Show(proposal.clone())];
    if failed {
        out.push(ProposalOut::OfferAgent);
    }
    (
        ProposalState::Shown {
            proposal,
            sanitised,
        },
        out,
    )
}
