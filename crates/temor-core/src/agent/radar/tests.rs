use super::*;
use crate::presence::{
    AgentState, PaneFocus, PresenceOut, Seen, StatusReport, StatusSource, WaitKind,
};
use crate::vocab::ids::{ExitCode, PaneId, Seq};
use crate::vocab::text::{Note, PluginId};

const SPIN: &str = "\u{2802} fixing tests";

fn fact(record: &str, kind: ProgramKind) -> RadarIn {
    RadarIn::Program(ProgramFact {
        record: record.into(),
        kind,
        message: Some(Note("m".into())),
    })
}

fn adapter(state: AgentState, seq: u64) -> RadarIn {
    RadarIn::Report(StatusReport {
        pane: PaneId(1),
        state,
        seq: Seq(seq),
        source: StatusSource::Adapter(PluginId("claude".into())),
        note: None,
    })
}

/// Runs `(time in seconds, input)` steps and returns the state shown after each.
fn run(focus: PaneFocus, steps: Vec<(u64, RadarIn)>) -> Vec<AgentState> {
    let params = RadarParams::default();
    let mut state = RadarState::new(focus);
    let mut shown = Vec::new();
    for (secs, input) in steps {
        state = radar_step(state, Millis(secs * 1000), input, &params).0;
        shown.push(state.state());
    }
    shown
}

use AgentState::{Done, Failed, Idle, Waiting, Working};
use PaneFocus::{Focused, Unfocused};
use RadarIn::{Focus, Process, PromptShown, Tick, Title};

#[test]
fn the_sources_follow_their_rules_over_time() {
    let running = Process(ProcessFact::AgentRunning);
    let blocked = |w| fact("", ProgramKind::Blocked(w));
    #[rustfmt::skip]
    let cases: Vec<(&str, PaneFocus, Vec<(u64, RadarIn)>, Vec<AgentState>)> = vec![
        ("osc 7501 working, blocked, done; unseen until focus", Unfocused,
            vec![(0, fact("", ProgramKind::Working)), (1, blocked(Some(WaitKind::Permission))), (2, fact("", ProgramKind::Done)), (3, Focus(Focused))],
            vec![Working, Waiting(WaitKind::Permission), Done(Seen::Unseen), Done(Seen::Seen)]),
        ("a blocked report with no kind asks a question", Focused,
            vec![(0, blocked(None))], vec![Waiting(WaitKind::Question)]),
        ("a new prompt clears working records", Focused,
            vec![(0, fact("", ProgramKind::Working)), (1, PromptShown)], vec![Working, Idle]),
        ("records: a child's error shows when the root says nothing; clearing the parent removes it", Focused,
            vec![(0, fact("build/test", ProgramKind::Error)), (1, fact("build", ProgramKind::Clear))],
            vec![Failed(Seen::Seen), Idle]),
        ("the root record wins over a child", Focused,
            vec![(0, fact("a", ProgramKind::Error)), (1, fact("", ProgramKind::Working))], vec![Failed(Seen::Seen), Working]),
        ("an adapter beats a notification while fresh", Unfocused,
            vec![(0, adapter(Working, 1)), (10, RadarIn::Notified(None))], vec![Working, Working]),
        ("the notification counts once the adapter has been quiet a minute", Unfocused,
            vec![(0, adapter(Idle, 1)), (61, RadarIn::Notified(None))], vec![Idle, Waiting(WaitKind::Question)]),
        ("a notification waits for you only when unfocused, and focus ends it", Unfocused,
            vec![(0, RadarIn::Notified(None)), (1, Focus(Focused))], vec![Waiting(WaitKind::Question), Idle]),
        ("a notification in a focused pane does nothing", Focused,
            vec![(0, RadarIn::Notified(None))], vec![Idle]),
        ("a spinner title is working for four seconds, only for a known agent", Focused,
            vec![(0, Title(SPIN.into())), (1, running.clone()), (2, Title(SPIN.into())), (5, Tick), (6, Tick)],
            vec![Idle, Idle, Working, Working, Idle]),
        ("a quiet title ends title-driven work at once", Focused,
            vec![(0, running.clone()), (1, Title(SPIN.into())), (2, Title("claude".into()))], vec![Idle, Working, Idle]),
        ("a title never overrides an adapter's working", Focused,
            vec![(0, running.clone()), (1, adapter(Working, 1)), (2, Title("claude".into()))], vec![Idle, Working, Working]),
        ("working with no word for fifteen minutes shows idle", Focused,
            vec![(0, fact("", ProgramKind::Working)), (899, Tick), (900, Tick)], vec![Working, Working, Idle]),
        ("exit while working: zero is done, non-zero failed", Unfocused,
            vec![(0, fact("", ProgramKind::Working)), (1, Process(ProcessFact::Exited(ExitCode(0))))], vec![Working, Done(Seen::Unseen)]),
        ("exit non-zero while waiting", Focused,
            vec![(0, adapter(Waiting(WaitKind::SignIn), 1)), (1, Process(ProcessFact::Exited(ExitCode(2))))], vec![Waiting(WaitKind::SignIn), Failed(Seen::Seen)]),
        ("an older adapter report is ignored", Focused,
            vec![(0, adapter(Working, 5)), (1, adapter(Idle, 4))], vec![Working, Working]),
    ];
    for (name, focus, steps, want) in cases {
        assert_eq!(run(focus, steps), want, "{name}");
    }
}

#[test]
fn outputs_carry_the_orb_and_one_notification() {
    let params = RadarParams::default();
    let state = RadarState::new(Unfocused);
    let (state, out) = radar_step(state, Millis(0), fact("", ProgramKind::Working), &params);
    assert_eq!(out[0], RadarOut::Orb(Orb::Working));
    let (_, out) = radar_step(
        state,
        Millis(1000),
        fact("", ProgramKind::Blocked(Some(WaitKind::Permission))),
        &params,
    );
    assert_eq!(out[0], RadarOut::Orb(Orb::WaitingForYou));
    assert!(out.iter().any(|o| matches!(
        o,
        RadarOut::Presence(PresenceOut::Notify { note: Some(Note(n)), .. }) if n == "m"
    )));
}

#[test]
fn title_hints_read_only_a_leading_spinner() {
    for (title, want) in [
        ("\u{280B} work", TitleHint::Busy),
        ("claude", TitleHint::Quiet),
        ("", TitleHint::Quiet),
        ("work \u{280B}", TitleHint::Quiet),
    ] {
        assert_eq!(title_hint(title), want, "{title:?}");
    }
}
