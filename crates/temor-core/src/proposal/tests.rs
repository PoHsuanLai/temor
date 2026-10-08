use super::{
    AsRun, Cleaned, FileChanges, Proposal, ProposalIn, ProposalOut, ProposalPhase, ProposalState,
    Sanitised, SanitisedLine, TrialAttempt, TrialId, TrialVerdict, UncheckedWhy, proposal_key,
    proposal_step, sanitise,
};
use crate::block::Taint;
use crate::key::{PassThrough, ShellKey};
use crate::vocab::ai::AiSwitch;
use crate::vocab::ids::{ExitCode, TurnRef};
use crate::vocab::text::{CommandText, ErrorLine};
use TrialAttempt::{AfterFailure, First};

const T: TurnRef = TurnRef(1);

fn id(attempt: TrialAttempt) -> TrialId {
    TrialId { turn: T, attempt }
}
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
fn cleaned(verdict: TrialVerdict) -> Cleaned {
    Cleaned {
        proposal: Proposal {
            text: line("ls -la"),
            turn: T,
            taint: Taint::Clean,
            verdict,
        },
        sanitised: Sanitised::Unchanged,
    }
}
fn at(phase: ProposalPhase) -> ProposalState {
    ProposalState { phase, typed: None }
}
fn asking(attempt: TrialAttempt) -> ProposalState {
    at(ProposalPhase::Asking(id(attempt)))
}
fn trialling(attempt: TrialAttempt) -> ProposalState {
    at(ProposalPhase::Trialling {
        id: id(attempt),
        line: line("ls -la"),
        sanitised: Sanitised::Unchanged,
        taint: Taint::Clean,
    })
}
fn shown(verdict: TrialVerdict) -> ProposalState {
    at(ProposalPhase::Shown(cleaned(verdict)))
}
fn typed() -> ProposalState {
    ProposalState {
        phase: ProposalPhase::Off,
        typed: Some(cleaned(clean())),
    }
}
fn answer(text: &str, turn: TurnRef) -> ProposalIn {
    ProposalIn::ModelAnswer {
        turn,
        text: text.into(),
        taint: Taint::Clean,
    }
}
fn result(attempt: TrialAttempt, verdict: TrialVerdict) -> ProposalIn {
    ProposalIn::TrialResult {
        id: id(attempt),
        verdict,
    }
}
fn ran(as_run: AsRun) -> ProposalOut {
    ProposalOut::Ran {
        typed: cleaned(clean()),
        as_run,
    }
}

#[test]
fn a_proposal_goes_from_ask_to_trial_to_prompt() {
    #[rustfmt::skip]
    let cases: Vec<(&str, ProposalState, ProposalIn, ProposalState, Vec<ProposalOut>)> = vec![
        ("ask calls the model", ProposalState::OFF, ProposalIn::Ask(T), asking(First),
            vec![ProposalOut::CallModel { turn: T, failed: None }]),
        ("an answer is cleaned and trialled", asking(First), answer("ls -la", T), trialling(First),
            vec![ProposalOut::RunTrial { id: id(First), line: line("ls -la") }]),
        ("an answer to an old ask is ignored", asking(First), answer("ls -la", TurnRef(0)), asking(First), vec![]),
        ("an answer that cleans to nothing", asking(First), answer("\n\u{1b}[0m \t", T), ProposalState::OFF,
            vec![ProposalOut::NothingToPropose]),
        ("the model failing", asking(First), ProposalIn::ModelFailed(T), ProposalState::OFF,
            vec![ProposalOut::NothingToPropose]),
        ("a clean trial shows the chip", trialling(First), result(First, clean()), shown(clean()),
            vec![ProposalOut::Show(cleaned(clean()).proposal)]),
        ("an unchecked trial shows the chip", trialling(First), result(First, TrialVerdict::Unchecked(UncheckedWhy::NoSandbox)),
            shown(TrialVerdict::Unchecked(UncheckedWhy::NoSandbox)),
            vec![ProposalOut::Show(cleaned(TrialVerdict::Unchecked(UncheckedWhy::NoSandbox)).proposal)]),
        ("a first failure goes back to the model once", trialling(First), result(First, failed()), asking(AfterFailure),
            vec![ProposalOut::CallModel { turn: T, failed: Some(ErrorLine("no such file".into())) }]),
        ("a second failure is shown with the agent offer", trialling(AfterFailure), result(AfterFailure, failed()), shown(failed()),
            vec![ProposalOut::Show(cleaned(failed()).proposal), ProposalOut::OfferAgent]),
        ("a late result of the first trial is dropped", trialling(AfterFailure), result(First, clean()), trialling(AfterFailure), vec![]),
        ("accept types it into the prompt", shown(clean()), ProposalIn::Accept, typed(),
            vec![ProposalOut::TypeIntoPrompt(line("ls -la"))]),
        ("running it as typed is logged", typed(), ProposalIn::LeftPrompt(CommandText("ls -la".into())), ProposalState::OFF,
            vec![ran(AsRun::AsProposed)]),
        ("running an edit is logged as edited", typed(), ProposalIn::LeftPrompt(CommandText("ls -l".into())), ProposalState::OFF,
            vec![ran(AsRun::Edited)]),
        ("running something else dismisses the chip", shown(clean()), ProposalIn::LeftPrompt(CommandText("make".into())), ProposalState::OFF,
            vec![ProposalOut::Dismissed]),
        ("accept before the trial does nothing", trialling(First), ProposalIn::Accept, trialling(First), vec![]),
        ("nothing moves while off", ProposalState::OFF, ProposalIn::Accept, ProposalState::OFF, vec![]),
    ];
    for (name, from, input, want, outs) in cases {
        let (state, out) = proposal_step(from, input, AiSwitch::On);
        assert_eq!(state, want, "{name}: state");
        assert_eq!(out, outs, "{name}: outputs");
    }
}

#[test]
fn a_stale_trial_result_is_never_shown_with_a_newer_command() {
    let t2 = TurnRef(2);
    let (state, _) = proposal_step(ProposalState::OFF, ProposalIn::Ask(T), AiSwitch::On);
    let (state, _) = proposal_step(state, answer("ls", T), AiSwitch::On);
    let (state, _) = proposal_step(state, ProposalIn::Ask(t2), AiSwitch::On);
    let (state, _) = proposal_step(state, answer("rm -rf ~/x", t2), AiSwitch::On);
    let before = state.clone();
    let (state, out) = proposal_step(state, result(First, clean()), AiSwitch::On);
    assert_eq!((state, out), (before, vec![]), "t1's result is dropped");
}

#[test]
fn a_typed_proposal_is_still_logged_after_a_new_ask_or_with_ai_off() {
    let (state, _) = proposal_step(typed(), ProposalIn::Ask(TurnRef(2)), AiSwitch::On);
    assert_eq!(
        state.typed,
        typed().typed,
        "asking again keeps the typed proposal"
    );
    let (_, out) = proposal_step(
        state,
        ProposalIn::LeftPrompt(CommandText("ls -la".into())),
        AiSwitch::On,
    );
    assert_eq!(out, vec![ProposalOut::Dismissed, ran(AsRun::AsProposed)]);
    let (_, out) = proposal_step(
        typed(),
        ProposalIn::LeftPrompt(CommandText("ls -la".into())),
        AiSwitch::Off,
    );
    assert_eq!(
        out,
        vec![ran(AsRun::AsProposed)],
        "AI off still logs the person's act"
    );
}

#[test]
fn with_ai_off_nothing_reaches_a_model() {
    let (state, out) = proposal_step(ProposalState::OFF, ProposalIn::Ask(T), AiSwitch::Off);
    assert_eq!((state, out), (ProposalState::OFF, vec![]));
    let (state, out) = proposal_step(asking(First), answer("ls", T), AiSwitch::Off);
    assert_eq!(
        (state, out),
        (ProposalState::OFF, vec![ProposalOut::Dismissed])
    );
}

#[test]
fn escape_and_ctrl_c_always_reach_the_shell() {
    let states = [
        ProposalState::OFF,
        asking(First),
        trialling(First),
        shown(clean()),
        typed(),
    ];
    for key in [ShellKey::Escape, ShellKey::Interrupt] {
        for from in states.clone() {
            let was_off = from == ProposalState::OFF;
            let was_typed = from.typed.clone();
            let (state, out, passed) = proposal_key(from, key);
            assert_eq!(passed, PassThrough(key), "{key:?}");
            assert_eq!(state.phase, ProposalPhase::Off, "{key:?}");
            let kept = if key == ShellKey::Escape {
                was_typed
            } else {
                None
            };
            assert_eq!(
                state.typed, kept,
                "{key:?}: Ctrl-C clears the line, Escape does not"
            );
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
    let (state, _) = proposal_step(asking(First), input, AiSwitch::On);
    let (state, out) = proposal_step(state, result(First, clean()), AiSwitch::On);
    let ProposalPhase::Shown(shown) = state.phase else {
        panic!("{state:?}")
    };
    assert_eq!(shown.proposal.taint, Taint::ReadUntrusted);
    assert_eq!(out, vec![ProposalOut::Show(shown.proposal)]);
}
