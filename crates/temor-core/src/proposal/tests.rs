use super::{
    AsRun, FileChanges, Proposal, ProposalIn, ProposalOut, ProposalState, Sanitised, SanitisedLine,
    TrialAttempt, TrialVerdict, UncheckedWhy, proposal_key, proposal_step, sanitise,
};
use crate::block::Taint;
use crate::key::{PassThrough, ShellKey};
use crate::vocab::ai::AiSwitch;
use crate::vocab::ids::{ExitCode, TurnRef};
use crate::vocab::text::{CommandText, ErrorLine};

const T: TurnRef = TurnRef(1);

fn line(text: &str) -> SanitisedLine {
    sanitise(text).0
}
fn clean() -> TrialVerdict {
    TrialVerdict::Clean {
        changes: FileChanges::default(),
    }
}
fn failed() -> TrialVerdict {
    TrialVerdict::Failed {
        exit: ExitCode(2),
        first_error: ErrorLine("no such file".into()),
    }
}
fn proposal(verdict: TrialVerdict) -> Proposal {
    Proposal {
        text: line("ls -la"),
        turn: T,
        taint: Taint::Clean,
        verdict,
    }
}
fn asking(attempt: TrialAttempt) -> ProposalState {
    ProposalState::Asking { turn: T, attempt }
}
fn trialling(attempt: TrialAttempt) -> ProposalState {
    ProposalState::Trialling {
        line: line("ls -la"),
        sanitised: Sanitised::Unchanged,
        turn: T,
        taint: Taint::Clean,
        attempt,
    }
}
fn shown(verdict: TrialVerdict) -> ProposalState {
    ProposalState::Shown {
        proposal: proposal(verdict),
        sanitised: Sanitised::Unchanged,
    }
}
fn typed() -> ProposalState {
    ProposalState::Typed {
        proposal: proposal(clean()),
        sanitised: Sanitised::Unchanged,
    }
}
fn answer(text: &str, turn: TurnRef) -> ProposalIn {
    ProposalIn::ModelAnswer {
        turn,
        text: text.into(),
        taint: Taint::Clean,
    }
}

#[test]
fn a_proposal_goes_from_ask_to_trial_to_prompt() {
    use TrialAttempt::{AfterFailure, First};
    #[rustfmt::skip]
    let cases: Vec<(&str, ProposalState, ProposalIn, ProposalState, Vec<ProposalOut>)> = vec![
        ("ask calls the model", ProposalState::Off, ProposalIn::Ask(T), asking(First),
            vec![ProposalOut::CallModel { turn: T, failed: None }]),
        ("an answer is cleaned and trialled", asking(First), answer("ls -la", T), trialling(First),
            vec![ProposalOut::RunTrial(line("ls -la"))]),
        ("an answer to an old ask is ignored", asking(First), answer("ls -la", TurnRef(0)), asking(First), vec![]),
        ("an answer that cleans to nothing", asking(First), answer("\n\u{1b}[0m \t", T), ProposalState::Off,
            vec![ProposalOut::NothingToPropose]),
        ("the model failing", asking(First), ProposalIn::ModelFailed(T), ProposalState::Off,
            vec![ProposalOut::NothingToPropose]),
        ("a clean trial shows the chip", trialling(First), ProposalIn::TrialResult(clean()), shown(clean()),
            vec![ProposalOut::Show(proposal(clean()))]),
        ("an unchecked trial shows the chip", trialling(First), ProposalIn::TrialResult(TrialVerdict::Unchecked(UncheckedWhy::NoSandbox)),
            shown(TrialVerdict::Unchecked(UncheckedWhy::NoSandbox)),
            vec![ProposalOut::Show(proposal(TrialVerdict::Unchecked(UncheckedWhy::NoSandbox)))]),
        ("a first failure goes back to the model once", trialling(First), ProposalIn::TrialResult(failed()), asking(AfterFailure),
            vec![ProposalOut::CallModel { turn: T, failed: Some(ErrorLine("no such file".into())) }]),
        ("a second failure is shown with the agent offer", trialling(AfterFailure), ProposalIn::TrialResult(failed()), shown(failed()),
            vec![ProposalOut::Show(proposal(failed())), ProposalOut::OfferAgent]),
        ("accept types it into the prompt", shown(clean()), ProposalIn::Accept, typed(),
            vec![ProposalOut::TypeIntoPrompt(line("ls -la"))]),
        ("running it as typed is logged", typed(), ProposalIn::LeftPrompt(CommandText("ls -la".into())), ProposalState::Off,
            vec![ProposalOut::Ran { proposal: proposal(clean()), sanitised: Sanitised::Unchanged, as_run: AsRun::AsProposed }]),
        ("running an edit is logged as edited", typed(), ProposalIn::LeftPrompt(CommandText("ls -l".into())), ProposalState::Off,
            vec![ProposalOut::Ran { proposal: proposal(clean()), sanitised: Sanitised::Unchanged, as_run: AsRun::Edited }]),
        ("running something else dismisses the chip", shown(clean()), ProposalIn::LeftPrompt(CommandText("make".into())), ProposalState::Off,
            vec![ProposalOut::Dismissed]),
        ("accept before the trial does nothing", trialling(First), ProposalIn::Accept, trialling(First), vec![]),
        ("nothing moves while off", ProposalState::Off, ProposalIn::Accept, ProposalState::Off, vec![]),
    ];
    for (name, from, input, want, outs) in cases {
        let (state, out) = proposal_step(from, input, AiSwitch::On);
        assert_eq!(state, want, "{name}: state");
        assert_eq!(out, outs, "{name}: outputs");
    }
}

#[test]
fn with_ai_off_nothing_reaches_a_model() {
    let (state, out) = proposal_step(ProposalState::Off, ProposalIn::Ask(T), AiSwitch::Off);
    assert_eq!((state, out), (ProposalState::Off, vec![]));
    let (state, out) = proposal_step(asking(TrialAttempt::First), answer("ls", T), AiSwitch::Off);
    assert_eq!(
        (state, out),
        (ProposalState::Off, vec![ProposalOut::Dismissed])
    );
}

#[test]
fn escape_and_ctrl_c_always_reach_the_shell() {
    let states = [
        ProposalState::Off,
        asking(TrialAttempt::First),
        trialling(TrialAttempt::First),
        shown(clean()),
        typed(),
    ];
    for key in [ShellKey::Escape, ShellKey::Interrupt] {
        for from in states.clone() {
            let was_off = from == ProposalState::Off;
            let (state, out, passed) = proposal_key(from, key);
            assert_eq!(passed, PassThrough(key), "{key:?}");
            assert_eq!(state, ProposalState::Off, "{key:?}");
            let want = if was_off {
                vec![]
            } else {
                vec![ProposalOut::Dismissed]
            };
            assert_eq!(out, want, "{key:?}");
        }
    }
}

#[test]
fn a_tainted_answer_keeps_its_taint() {
    let input = ProposalIn::ModelAnswer {
        turn: T,
        text: "rm x".into(),
        taint: Taint::ReadUntrusted,
    };
    let (state, _) = proposal_step(asking(TrialAttempt::First), input, AiSwitch::On);
    let (state, out) = proposal_step(state, ProposalIn::TrialResult(clean()), AiSwitch::On);
    let ProposalState::Shown { proposal, .. } = state else {
        panic!("{state:?}")
    };
    assert_eq!(proposal.taint, Taint::ReadUntrusted);
    assert_eq!(out, vec![ProposalOut::Show(proposal)]);
}
