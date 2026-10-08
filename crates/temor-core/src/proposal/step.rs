//! `proposal_step`: from the person asking, through the model and the trial, to the line typed
//! into their prompt, where their own Enter is the approval (§8.3).
//!
//! Asked for, never offered: only `Ask` starts it. A failed trial goes back to the model once;
//! a second failure is shown with an offer to ask an agent. Escape and Ctrl-C go through
//! [`proposal_key`], which always hands the key on to the shell.

use super::model::{
    Cleaned, Proposal, ProposalPhase, ProposalState, TrialAttempt, TrialId, TrialVerdict,
};
use super::sanitise::{Sanitised, SanitisedLine, sanitise};
use crate::block::Taint;
use crate::key::{PassThrough, ShellKey};
use crate::vocab::ai::AiSwitch;
use crate::vocab::ids::TurnRef;
use crate::vocab::text::{CommandText, ErrorLine};
use ds_core::word::Word;

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
    /// A trial run finished.
    TrialResult {
        /// The trial, as [`ProposalOut::RunTrial`] named it.
        id: TrialId,
        /// What it found.
        verdict: TrialVerdict,
    },
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
    /// Trial-run this line in the sandbox, and answer with this id.
    RunTrial {
        /// The trial.
        id: TrialId,
        /// The line.
        line: SanitisedLine,
    },
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
    /// The person ran a typed proposal: log it as their act.
    Ran {
        /// What was proposed, and what cleaning removed.
        typed: Cleaned,
        /// Whether they changed it.
        as_run: AsRun,
    },
}

/// The proposal after `input`. Answers and trial results that are not for the ask under way are
/// dropped. While `ai` is off nothing reaches a model: the flow is dropped, but a proposal
/// already typed is still logged if the person runs it.
pub fn proposal_step(
    state: ProposalState,
    input: ProposalIn,
    ai: AiSwitch,
) -> (ProposalState, Vec<ProposalOut>) {
    if let ProposalIn::LeftPrompt(command) = input {
        return left_prompt(state, &command);
    }
    if ai == AiSwitch::Off {
        let out = dismissed(&state.phase);
        let state = ProposalState {
            phase: ProposalPhase::Off,
            ..state
        };
        return (state, out);
    }
    let ProposalState { phase, typed } = state;
    let (phase, typed, out) = flow(phase, typed, input);
    (ProposalState { phase, typed }, out)
}

/// Escape or Ctrl-C while a proposal is up: the chip goes, and the key goes on to the shell
/// whatever the state, so Ctrl-C still clears the line and Escape still means what the shell
/// makes it mean. Ctrl-C clears the line, so a typed proposal goes with it; after Escape it is
/// still in the line, and running it is still logged.
pub fn proposal_key(
    state: ProposalState,
    key: ShellKey,
) -> (ProposalState, Vec<ProposalOut>, PassThrough) {
    let showing = state.phase != ProposalPhase::Off || state.typed.is_some();
    let out = if showing {
        vec![ProposalOut::Dismissed]
    } else {
        vec![]
    };
    let typed = match key {
        ShellKey::Escape => state.typed,
        ShellKey::Interrupt => None,
    };
    let state = ProposalState {
        phase: ProposalPhase::Off,
        typed,
    };
    (state, out, PassThrough(key))
}

fn flow(
    phase: ProposalPhase,
    typed: Option<Cleaned>,
    input: ProposalIn,
) -> (ProposalPhase, Option<Cleaned>, Vec<ProposalOut>) {
    use ProposalPhase as P;
    match (phase, input) {
        (_, ProposalIn::Ask(turn)) => (
            P::Asking(TrialId {
                turn,
                attempt: TrialAttempt::First,
            }),
            typed,
            vec![ProposalOut::CallModel { turn, failed: None }],
        ),
        (P::Asking(id), ProposalIn::ModelAnswer { turn, text, taint }) if turn == id.turn => {
            let (line, sanitised) = sanitise(&text);
            if line.as_str().trim().is_empty() {
                return (P::Off, typed, vec![ProposalOut::NothingToPropose]);
            }
            let out = vec![ProposalOut::RunTrial {
                id,
                line: line.clone(),
            }];
            let phase = P::Trialling {
                id,
                line,
                sanitised,
                taint,
            };
            (phase, typed, out)
        }
        (P::Asking(id), ProposalIn::ModelFailed(turn)) if turn == id.turn => {
            (P::Off, typed, vec![ProposalOut::NothingToPropose])
        }
        (
            P::Trialling {
                id,
                line,
                sanitised,
                taint,
            },
            ProposalIn::TrialResult { id: of, verdict },
        ) if of == id => {
            let (phase, out) = trialled(id, line, sanitised, taint, verdict);
            (phase, typed, out)
        }
        (P::Shown(shown), ProposalIn::Accept) => {
            let out = vec![ProposalOut::TypeIntoPrompt(shown.proposal.text.clone())];
            (P::Off, Some(shown), out)
        }
        (phase, _) => (phase, typed, vec![]),
    }
}

/// The shell left the prompt: a typed proposal ran (as typed or edited), and whatever was under
/// way no longer fits the prompt.
fn left_prompt(state: ProposalState, command: &CommandText) -> (ProposalState, Vec<ProposalOut>) {
    let mut out = dismissed(&state.phase);
    if let Some(typed) = state.typed {
        let as_run = if command.as_str() == typed.proposal.text.as_str() {
            AsRun::AsProposed
        } else {
            AsRun::Edited
        };
        out.push(ProposalOut::Ran { typed, as_run });
    }
    (ProposalState::OFF, out)
}

fn dismissed(phase: &ProposalPhase) -> Vec<ProposalOut> {
    match phase {
        ProposalPhase::Off => vec![],
        _ => vec![ProposalOut::Dismissed],
    }
}

fn trialled(
    id: TrialId,
    line: SanitisedLine,
    sanitised: Sanitised,
    taint: Taint,
    verdict: TrialVerdict,
) -> (ProposalPhase, Vec<ProposalOut>) {
    if let (TrialVerdict::Failed { first_error, .. }, TrialAttempt::First) = (&verdict, id.attempt)
    {
        let out = vec![ProposalOut::CallModel {
            turn: id.turn,
            failed: Some(first_error.clone()),
        }];
        let again = TrialId {
            turn: id.turn,
            attempt: TrialAttempt::AfterFailure,
        };
        return (ProposalPhase::Asking(again), out);
    }
    let failed = matches!(verdict, TrialVerdict::Failed { .. });
    let proposal = Proposal {
        text: line,
        turn: id.turn,
        taint,
        verdict,
    };
    let mut out = vec![ProposalOut::Show(proposal.clone())];
    if failed {
        out.push(ProposalOut::OfferAgent);
    }
    (
        ProposalPhase::Shown(Cleaned {
            proposal,
            sanitised,
        }),
        out,
    )
}
