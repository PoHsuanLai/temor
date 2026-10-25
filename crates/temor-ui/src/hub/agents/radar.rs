//! The window's radar (PLAN §8.1): one `RadarState` per pane, fed from what the pane's events and
//! the pane socket say, on a clock the caller passes in. Pure bookkeeping: the hub gives it the
//! events and the time, and reads back the states.

use std::collections::BTreeMap;

use temor_core::agent::radar::{
    Millis, ProcessFact, RadarIn, RadarOut, RadarParams, RadarState, radar_step,
};
use temor_core::agent::seen::{CommandLine, agent_of, is_known_agent};
use temor_core::presence::{AgentState, PaneFocus, PresenceOut, Seen};
use temor_core::vocab::ids::PaneId;
use temor_core::vocab::text::Note;
use temor_pane::PaneEvent;
use temor_vt::{ProgramState, PromptEvent};

use super::convert::{exit_code, note_of, program_fact, status_report};
use super::ending::{Via, front_of};
use super::naming::Named;

/// Whether a pane is known to run an agent: by its process, or because it reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Known {
    No,
    Yes,
}

#[derive(Debug, Clone)]
struct Entry {
    state: RadarState,
    known: Known,
    /// A known agent's name was seen (its process, or what it reported): plugins adapt to these
    /// panes only, never to a build tool that reports its progress.
    named: Option<Named>,
}

/// A pane that needs the person while its window is not the one in use: the desktop's
/// notification is made from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub pane: PaneId,
    pub state: AgentState,
    /// The agent's own words.
    pub note: Option<Note>,
}

/// Every pane's radar.
#[derive(Debug, Clone)]
pub struct Radar {
    params: RadarParams,
    panes: BTreeMap<PaneId, Entry>,
    /// The pane that has the keyboard; a new entry starts with the right focus.
    focused: Option<PaneId>,
    alerts: Vec<Alert>,
}

/// Whether feeding the radar changed what the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Changed {
    Yes,
    No,
}

impl Radar {
    /// A radar with nothing reported.
    pub fn new(params: RadarParams) -> Radar {
        Radar {
            params,
            panes: BTreeMap::new(),
            focused: None,
            alerts: Vec::new(),
        }
    }

    fn entry(&mut self, pane: PaneId) -> &mut Entry {
        let focus = match self.focused == Some(pane) {
            true => PaneFocus::Focused,
            false => PaneFocus::Unfocused,
        };
        self.panes.entry(pane).or_insert_with(|| Entry {
            state: RadarState::new(focus),
            known: Known::No,
            named: None,
        })
    }

    fn feed(&mut self, pane: PaneId, now: Millis, input: RadarIn) -> Changed {
        let params = self.params;
        let entry = self.entry(pane);
        let before = entry.state.state();
        let (state, outs) = radar_step(entry.state.clone(), now, input, &params);
        entry.state = state;
        let after = entry.state.state();
        for out in outs {
            if let RadarOut::Presence(PresenceOut::Notify { state, note }) = out {
                self.alerts.push(Alert { pane, state, note });
            }
        }
        match before == after {
            true => Changed::No,
            false => Changed::Yes,
        }
    }

    fn know(&mut self, pane: PaneId, app: Option<&str>, via: Via, command: Option<&CommandLine>) {
        let entry = self.entry(pane);
        entry.known = Known::Yes;
        if let Some(app) = app.filter(|app| is_known_agent(app)) {
            Named::learn(&mut entry.named, app, via, command);
        }
    }

    /// `pane`'s agent is no longer known when `gone` says so of the way it was seen.
    fn leave(&mut self, pane: PaneId, gone: impl FnOnce(Via) -> bool) {
        if let Some(entry) = self.panes.get_mut(&pane)
            && entry.named.as_ref().is_some_and(|n| gone(n.via))
        {
            entry.named = None;
        }
    }

    /// What the agent process seen in `pane` was started with, if its line could be read.
    pub fn command_of(&self, pane: PaneId) -> Option<&CommandLine> {
        self.panes.get(&pane)?.named.as_ref()?.command.as_ref()
    }

    /// The known agent program seen in `pane` (lowercase), if any.
    pub fn agent_of(&self, pane: PaneId) -> Option<&str> {
        let entry = self.panes.get(&pane)?;
        entry.named.as_ref().map(|n| n.program.as_str())
    }

    /// The pane that has the keyboard, as the radar last heard.
    pub fn focused(&self) -> Option<PaneId> {
        self.focused
    }

    fn is_known(&self, pane: PaneId) -> bool {
        self.panes.get(&pane).is_some_and(|e| e.known == Known::Yes)
    }

    /// What one event from `pane` means for its radar. Only an event that is about agents moves
    /// anything; bells and notifications count only in a pane known to run an agent.
    pub fn observe(&mut self, pane: PaneId, now: Millis, event: &PaneEvent) -> Changed {
        match event {
            PaneEvent::Program(status) => {
                self.know(pane, Some(status.app.as_str()), Via::Report, None);
                let changed = self.feed(pane, now, RadarIn::Program(program_fact(status)));
                // Clearing its status is a program saying it has ended.
                if matches!(status.state, ProgramState::Clear) {
                    self.leave(pane, |via| via == Via::Report);
                }
                changed
            }
            #[cfg(unix)]
            PaneEvent::Reported(line) => {
                self.know(pane, line.app.as_deref(), Via::Report, None);
                self.feed(pane, now, RadarIn::Report(status_report(pane, line)))
            }
            PaneEvent::Foreground(Some(seen))
                if agent_of(&seen.name, seen.command.as_ref()).is_some() =>
            {
                let sighting = agent_of(&seen.name, seen.command.as_ref());
                let program = sighting.as_ref().map(|s| s.agent.as_str());
                // Only the program and its arguments are kept: what runs it is no one's business.
                let command = sighting
                    .as_ref()
                    .zip(seen.command.as_ref())
                    .and_then(|(s, command)| command.from_program(s.program_at));
                self.know(pane, program, Via::Process, command.as_ref());
                self.feed(pane, now, RadarIn::Process(ProcessFact::AgentRunning))
            }
            // The agent seen by its process left the front of the pane, or a shell came back
            // after an agent that only reported: it has exited.
            PaneEvent::Foreground(seen) => {
                let front = front_of(seen.as_ref().map(|seen| seen.name.as_str()));
                self.leave(pane, |via| via.gone_with(front));
                Changed::No
            }
            PaneEvent::Title(title) if self.panes.contains_key(&pane) => {
                self.feed(pane, now, RadarIn::Title(title.clone()))
            }
            PaneEvent::Bell if self.is_known(pane) => self.feed(pane, now, RadarIn::Notified(None)),
            PaneEvent::Notification(note) if self.is_known(pane) => {
                let words = match note.body.is_empty() {
                    true => &note.title,
                    false => &note.body,
                };
                self.feed(pane, now, RadarIn::Notified(note_of(words)))
            }
            PaneEvent::Prompt(PromptEvent::PromptStart(_)) if self.panes.contains_key(&pane) => {
                self.feed(pane, now, RadarIn::PromptShown)
            }
            PaneEvent::Exited(exit) if self.panes.contains_key(&pane) => self.feed(
                pane,
                now,
                RadarIn::Process(ProcessFact::Exited(exit_code(*exit))),
            ),
            _ => Changed::No,
        }
    }

    /// The pane that has the keyboard changed: every pane's focus follows.
    pub fn set_focus(&mut self, now: Millis, focused: Option<PaneId>) -> Changed {
        self.focused = focused;
        let panes: Vec<PaneId> = self.panes.keys().copied().collect();
        let mut changed = Changed::No;
        for pane in panes {
            let focus = match Some(pane) == focused {
                true => PaneFocus::Focused,
                false => PaneFocus::Unfocused,
            };
            let current = self.panes.get(&pane).map(|e| e.state.presence.focus);
            if current != Some(focus) && self.feed(pane, now, RadarIn::Focus(focus)) == Changed::Yes
            {
                changed = Changed::Yes;
            }
        }
        changed
    }

    /// Time passed: stale and title-driven states settle. Called about once a second.
    pub fn tick(&mut self, now: Millis) -> Changed {
        let panes: Vec<PaneId> = self.panes.keys().copied().collect();
        let mut changed = Changed::No;
        for pane in panes {
            if self.feed(pane, now, RadarIn::Tick) == Changed::Yes {
                changed = Changed::Yes;
            }
        }
        changed
    }

    /// The pane is gone.
    pub fn forget(&mut self, pane: PaneId) {
        self.panes.remove(&pane);
    }

    /// What `pane` shows.
    pub fn state_of(&self, pane: PaneId) -> AgentState {
        self.panes
            .get(&pane)
            .map_or(AgentState::Idle, |e| e.state.state())
    }

    /// The most urgent state among `panes`: waiting, then an unseen result, then working.
    pub fn most_urgent(&self, panes: impl IntoIterator<Item = PaneId>) -> AgentState {
        panes
            .into_iter()
            .map(|pane| self.state_of(pane))
            .max_by_key(|state| urgency(*state))
            .unwrap_or(AgentState::Idle)
    }

    /// The panes waiting for the person, in pane order.
    pub fn waiting(&self) -> Vec<PaneId> {
        self.panes
            .iter()
            .filter(|(_, e)| matches!(e.state.state(), AgentState::Waiting(_)))
            .map(|(pane, _)| *pane)
            .collect()
    }

    /// The next waiting pane after `after` in pane order, wrapping round to the first.
    pub fn next_waiting(&self, after: Option<PaneId>) -> Option<PaneId> {
        let waiting = self.waiting();
        waiting
            .iter()
            .copied()
            .find(|pane| after.is_none_or(|a| *pane > a))
            .or_else(|| waiting.first().copied())
    }

    /// The notifications to make now; taking them clears them.
    pub fn take_alerts(&mut self) -> Vec<Alert> {
        std::mem::take(&mut self.alerts)
    }
}

fn urgency(state: AgentState) -> u8 {
    match state {
        AgentState::Waiting(_) => 6,
        AgentState::Failed(Seen::Unseen) => 5,
        AgentState::Done(Seen::Unseen) => 4,
        AgentState::Working => 3,
        AgentState::Failed(Seen::Seen) => 2,
        AgentState::Done(Seen::Seen) => 1,
        AgentState::Idle => 0,
    }
}
