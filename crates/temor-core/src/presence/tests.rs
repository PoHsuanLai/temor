use super::{
    AgentState, PaneFocus, PresenceIn, PresenceOut, PresenceState, Seen, StatusReport,
    StatusSource, WaitKind, attention, companion_presence, presence_step,
};
use crate::vocab::ids::{ExitCode, PaneId, Seq};
use crate::vocab::text::Note;
use ds_core::vocab::{CompanionPresence, Emphasis};
use std::collections::BTreeMap;

fn report(state: AgentState, seq: u64) -> PresenceIn {
    PresenceIn::Report(StatusReport {
        pane: PaneId(1),
        state,
        seq: Seq(seq),
        source: StatusSource::ProgramStatus,
        note: Some(Note("hi".into())),
    })
}

fn at(state: AgentState, focus: PaneFocus, newest: u64) -> PresenceState {
    PresenceState {
        state,
        focus,
        newest: BTreeMap::from([(StatusSource::ProgramStatus, Seq(newest))]),
    }
}

#[test]
fn presence_follows_reports_focus_prompts_and_exit() {
    use AgentState::{Done, Failed, Idle, Waiting, Working};
    use PaneFocus::{Focused, Unfocused};
    let note = Some(Note("hi".into()));
    #[rustfmt::skip]
    let cases: Vec<(&str, PresenceState, PresenceIn, AgentState, Vec<PresenceOut>)> = vec![
        ("a newer report applies", at(Idle, Focused, 1), report(Working, 2), Working, vec![PresenceOut::Changed(Working)]),
        ("a stale seq is ignored", at(Working, Focused, 5), report(Idle, 4), Working, vec![]),
        ("an equal seq is ignored", at(Working, Focused, 5), report(Idle, 5), Working, vec![]),
        ("another source has its own sequence", at(Working, Focused, 5),
            PresenceIn::Report(StatusReport { pane: PaneId(1), state: Idle, seq: Seq(1), source: StatusSource::Docket, note: None }),
            Idle, vec![PresenceOut::Changed(Idle)]),
        ("waiting while unfocused notifies with the note", at(Working, Unfocused, 1), report(Waiting(WaitKind::Permission), 2),
            Waiting(WaitKind::Permission),
            vec![PresenceOut::Changed(Waiting(WaitKind::Permission)), PresenceOut::Notify { state: Waiting(WaitKind::Permission), note: note.clone() }]),
        ("waiting while focused does not notify", at(Working, Focused, 1), report(Waiting(WaitKind::Question), 2),
            Waiting(WaitKind::Question), vec![PresenceOut::Changed(Waiting(WaitKind::Question))]),
        ("done while focused is seen at once", at(Working, Focused, 1), report(Done(Seen::Unseen), 2),
            Done(Seen::Seen), vec![PresenceOut::Changed(Done(Seen::Seen))]),
        ("done while unfocused stays unseen", at(Working, Unfocused, 1), report(Done(Seen::Seen), 2),
            Done(Seen::Unseen), vec![PresenceOut::Changed(Done(Seen::Unseen)), PresenceOut::Notify { state: Done(Seen::Unseen), note: note.clone() }]),
        ("focus turns done into seen", at(Done(Seen::Unseen), Unfocused, 1), PresenceIn::Focus(Focused),
            Done(Seen::Seen), vec![PresenceOut::Changed(Done(Seen::Seen))]),
        ("focus turns failed into seen", at(Failed(Seen::Unseen), Unfocused, 1), PresenceIn::Focus(Focused),
            Failed(Seen::Seen), vec![PresenceOut::Changed(Failed(Seen::Seen))]),
        ("focus leaves working alone", at(Working, Unfocused, 1), PresenceIn::Focus(Focused), Working, vec![]),
        ("a prompt clears working", at(Working, Unfocused, 1), PresenceIn::PromptShown, Idle, vec![PresenceOut::Changed(Idle)]),
        ("a prompt clears waiting", at(Waiting(WaitKind::SignIn), Unfocused, 1), PresenceIn::PromptShown, Idle, vec![PresenceOut::Changed(Idle)]),
        ("a prompt keeps an unseen result", at(Done(Seen::Unseen), Unfocused, 1), PresenceIn::PromptShown, Done(Seen::Unseen), vec![]),
        ("exit 0 while working is done", at(Working, Unfocused, 1), PresenceIn::ProcessExited(ExitCode(0)),
            Done(Seen::Unseen), vec![PresenceOut::Changed(Done(Seen::Unseen)), PresenceOut::Notify { state: Done(Seen::Unseen), note: None }]),
        ("a failing exit while working is failed", at(Working, Focused, 1), PresenceIn::ProcessExited(ExitCode(1)),
            Failed(Seen::Seen), vec![PresenceOut::Changed(Failed(Seen::Seen))]),
        ("exit while idle changes nothing", at(Idle, Unfocused, 1), PresenceIn::ProcessExited(ExitCode(1)), Idle, vec![]),
        ("exit keeps an unseen result and does not notify again", at(Done(Seen::Unseen), Unfocused, 1),
            PresenceIn::ProcessExited(ExitCode(0)), Done(Seen::Unseen), vec![]),
    ];
    for (name, from, input, want, outs) in cases {
        let (state, out) = presence_step(from, input);
        assert_eq!(state.state, want, "{name}: state");
        assert_eq!(out, outs, "{name}: outputs");
    }
}

#[test]
fn a_report_records_its_sequence() {
    let (state, _) = presence_step(
        at(AgentState::Idle, PaneFocus::Focused, 1),
        report(AgentState::Working, 7),
    );
    assert_eq!(
        state.newest.get(&StatusSource::ProgramStatus),
        Some(&Seq(7))
    );
}

#[test]
fn agent_states_map_to_the_companion_orb_and_the_unread_dot() {
    use AgentState::{Done, Failed, Idle, Waiting, Working};
    #[rustfmt::skip]
    const CASES: &[(AgentState, CompanionPresence, Emphasis)] = &[
        (Idle,                         CompanionPresence::Idle,    Emphasis::Plain),
        (Working,                      CompanionPresence::Working, Emphasis::Plain),
        (Waiting(WaitKind::Permission), CompanionPresence::Waiting, Emphasis::Strong),
        (Done(Seen::Unseen),           CompanionPresence::Idle,    Emphasis::Strong),
        (Done(Seen::Seen),             CompanionPresence::Idle,    Emphasis::Plain),
        (Failed(Seen::Unseen),         CompanionPresence::Idle,    Emphasis::Strong),
        (Failed(Seen::Seen),           CompanionPresence::Idle,    Emphasis::Plain),
    ];
    for &(state, orb, weight) in CASES {
        assert_eq!(companion_presence(state), orb, "{state:?}");
        assert_eq!(attention(state), weight, "{state:?}");
    }
}
