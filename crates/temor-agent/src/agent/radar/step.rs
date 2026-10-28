//! `radar_step`: one pane's agent status from every source, with precedence and timing.
//!
//! Rules (all deterministic, no guessing from words):
//! - A stronger source silences weaker ones while it is fresh: OSC 7501, then adapters, then
//!   docket, then notifications and title hints. When it goes quiet for `precedence_hold`, the
//!   weaker ones count again.
//! - OSC 7501 records are kept per id; a new prompt or the program exiting drops working and
//!   blocked records; finished records stay until the pane is seen.
//! - A notification or bell makes an unfocused pane Waiting until it is focused or a prompt
//!   comes up.
//! - A spinner title counts as Working for `title_hold` after it last changed, only while a
//!   known agent runs in the pane; when it stops, the pane goes idle.
//! - A source's Working with no report for `stale_working` shows idle again.
//! - The process exiting while Working or Waiting is Done or Failed by its exit code.

use std::collections::BTreeMap;

use super::hint::title_hint;
use super::model::{Millis, ProcessFact, RadarIn, RadarOut, RadarParams};
use super::records::Records;
use crate::presence::{
    AgentState, PaneFocus, PresenceIn, PresenceState, StatusReport, StatusSource, WaitKind,
    presence_step,
};
use temor_core::vocab::ids::Seq;

/// One pane's radar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadarState {
    /// The lower-level machine: state, focus, per-source sequence numbers.
    pub presence: PresenceState,
    records: Records,
    fresh: BTreeMap<StatusSource, Millis>,
    /// Who set the state now shown, and when it last spoke.
    via: Option<(StatusSource, Millis)>,
    /// Whether a known agent program runs in the pane.
    agent_running: bool,
    title_busy_until: Option<Millis>,
    next_seq: u64,
}

impl RadarState {
    /// A pane with nothing reported.
    pub fn new(focus: PaneFocus) -> RadarState {
        RadarState {
            presence: PresenceState::new(focus),
            records: Records::default(),
            fresh: BTreeMap::new(),
            via: None,
            agent_running: false,
            title_busy_until: None,
            next_seq: 1,
        }
    }

    /// What the pane shows.
    pub fn state(&self) -> AgentState {
        self.presence.state
    }
}

/// The radar after `input` at `now`.
pub fn radar_step(
    state: RadarState,
    now: Millis,
    input: RadarIn,
    params: &RadarParams,
) -> (RadarState, Vec<RadarOut>) {
    let mut state = state;
    let mut presence_in = Vec::new();
    match input {
        RadarIn::Program(fact) => {
            let source = StatusSource::ProgramStatus;
            if !outranked(&state, &source, now, params) {
                state.records.apply(fact, now);
                let (shown, note) = state.records.shown();
                presence_in.push(report(&mut state, source, now, shown, note));
            }
        }
        RadarIn::Report(r) if r.source == StatusSource::Notification => {}
        RadarIn::Report(r) => {
            if !outranked(&state, &r.source, now, params) {
                state.fresh.insert(r.source.clone(), now);
                state.via = Some((r.source.clone(), now));
                presence_in.push(PresenceIn::Report(r));
            }
        }
        RadarIn::Notified(note) => {
            let weak = StatusSource::Notification;
            if state.presence.focus == PaneFocus::Unfocused
                && !outranked(&state, &weak, now, params)
            {
                let waiting = AgentState::Waiting(WaitKind::Question);
                presence_in.push(report(&mut state, weak, now, waiting, note));
            }
        }
        RadarIn::Title(title) => title_step(&mut state, now, params, &title, &mut presence_in),
        RadarIn::Process(ProcessFact::AgentRunning) => state.agent_running = true,
        RadarIn::Process(ProcessFact::Exited(code)) => {
            state.agent_running = false;
            state.records.prompt_shown();
            state.title_busy_until = None;
            presence_in.push(PresenceIn::ProcessExited(code));
        }
        RadarIn::PromptShown => {
            state.records.prompt_shown();
            state.title_busy_until = None;
            presence_in.push(PresenceIn::PromptShown);
        }
        RadarIn::Focus(focus) => {
            if focus == PaneFocus::Focused {
                state.records.seen();
            }
            presence_in.push(PresenceIn::Focus(focus));
            let noticed = matches!(&state.via, Some((StatusSource::Notification, _)));
            if focus == PaneFocus::Focused
                && noticed
                && matches!(state.presence.state, AgentState::Waiting(_))
            {
                presence_in.push(report(
                    &mut state,
                    StatusSource::Notification,
                    now,
                    AgentState::Idle,
                    None,
                ));
            }
        }
        RadarIn::Tick => tick_step(&mut state, now, params, &mut presence_in),
    }
    let mut out = Vec::new();
    for input in presence_in {
        let (next, outs) = presence_step(state.presence.clone(), input);
        state.presence = next;
        out.extend(outs.into_iter().map(RadarOut::Presence));
    }
    (state, out)
}

/// A report the radar makes itself, with its own sequence number.
fn report(
    state: &mut RadarState,
    source: StatusSource,
    now: Millis,
    shown: AgentState,
    note: Option<temor_core::vocab::text::Note>,
) -> PresenceIn {
    let seq = Seq(state.next_seq);
    state.next_seq += 1;
    state.fresh.insert(source.clone(), now);
    state.via = Some((source.clone(), now));
    PresenceIn::Report(StatusReport {
        // The radar is per pane and `presence_step` never reads the id.
        pane: temor_core::vocab::ids::PaneId(0),
        state: shown,
        seq,
        source,
        note,
    })
}

/// Whether a stronger source reported within the hold.
fn outranked(state: &RadarState, source: &StatusSource, now: Millis, params: &RadarParams) -> bool {
    state.fresh.iter().any(|(other, at)| {
        strength(other) < strength(source) && now.since(*at) < params.precedence_hold.0
    })
}

/// Lower is stronger: the order of §8.1's source list.
fn strength(source: &StatusSource) -> u8 {
    match source {
        StatusSource::ProgramStatus => 0,
        StatusSource::Adapter(_) => 1,
        StatusSource::Docket => 2,
        StatusSource::Notification => 3,
    }
}

fn title_step(
    state: &mut RadarState,
    now: Millis,
    params: &RadarParams,
    title: &str,
    out: &mut Vec<PresenceIn>,
) {
    let weak = StatusSource::Notification;
    if !state.agent_running || outranked(state, &weak, now, params) {
        return;
    }
    if !title_hint(title) {
        stop_title_work(state, now, out);
        return;
    }
    state.title_busy_until = Some(Millis(now.0 + params.title_hold.0));
    if let Some((StatusSource::Notification, at)) = &mut state.via {
        *at = now;
    }
    if state.presence.state == AgentState::Idle {
        out.push(report(state, weak, now, AgentState::Working, None));
    }
}

/// A title-driven Working ends; one a stronger source set is left alone.
fn stop_title_work(state: &mut RadarState, now: Millis, out: &mut Vec<PresenceIn>) {
    let by_title = matches!(&state.via, Some((StatusSource::Notification, _)));
    if state.title_busy_until.take().is_some()
        && by_title
        && state.presence.state == AgentState::Working
    {
        out.push(report(
            state,
            StatusSource::Notification,
            now,
            AgentState::Idle,
            None,
        ));
    }
}

fn tick_step(state: &mut RadarState, now: Millis, params: &RadarParams, out: &mut Vec<PresenceIn>) {
    if state.title_busy_until.is_some_and(|until| now >= until) {
        stop_title_work(state, now, out);
    }
    let silent = state
        .via
        .as_ref()
        .is_some_and(|(_, at)| now.since(*at) >= params.stale_working.0);
    if silent && state.presence.state == AgentState::Working {
        // Clear what the source left behind, and show idle from the radar's own sequence.
        state.records = Records::default();
        out.push(report(
            state,
            StatusSource::Notification,
            now,
            AgentState::Idle,
            None,
        ));
    }
}
