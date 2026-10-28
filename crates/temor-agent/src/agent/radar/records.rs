//! OSC 7501 records: one per id, replaced whole, cleared with their children (the protocol's
//! rules), and reduced to the one state the pane shows.

use std::collections::BTreeMap;

use super::model::{Millis, ProgramFact, ProgramKind};
use crate::presence::{AgentState, Seen, WaitKind};
use temor_core::vocab::text::Note;

/// The most records kept; the one updated longest ago goes first.
const MAX_RECORDS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    kind: ProgramKind,
    message: Option<Note>,
    updated: Millis,
}

/// A pane's records.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Records(BTreeMap<String, Record>);

impl Records {
    /// Applies one report.
    pub(super) fn apply(&mut self, fact: ProgramFact, now: Millis) {
        if fact.kind == ProgramKind::Clear {
            let id = fact.record;
            self.0.retain(|key, _| {
                !(id.is_empty() || *key == id || key.starts_with(&format!("{id}/")))
            });
            return;
        }
        if self.0.len() >= MAX_RECORDS && !self.0.contains_key(&fact.record) {
            let oldest = self
                .0
                .iter()
                .min_by_key(|(_, r)| r.updated)
                .map(|(k, _)| k.clone());
            if let Some(key) = oldest {
                self.0.remove(&key);
            }
        }
        self.0.insert(
            fact.record,
            Record {
                kind: fact.kind,
                message: fact.message,
                updated: now,
            },
        );
    }

    /// A new prompt, or the program exiting: working and blocked records go, idle ones too.
    pub(super) fn prompt_shown(&mut self) {
        self.0
            .retain(|_, r| matches!(r.kind, ProgramKind::Done | ProgramKind::Error));
    }

    /// The person looked: finished records go.
    pub(super) fn seen(&mut self) {
        self.0
            .retain(|_, r| !matches!(r.kind, ProgramKind::Done | ProgramKind::Error));
    }

    /// The state to show and its message. The root record decides when there is one; else the
    /// most urgent of the rest (blocked, then error, working, done, idle). No records: idle.
    pub(super) fn shown(&self) -> (AgentState, Option<Note>) {
        let pick = self
            .0
            .get("")
            .or_else(|| self.0.values().max_by_key(|r| rank(r.kind)));
        match pick {
            Some(r) => (state_of(r.kind), r.message.clone()),
            None => (AgentState::Idle, None),
        }
    }
}

fn rank(kind: ProgramKind) -> u8 {
    match kind {
        ProgramKind::Blocked(_) => 5,
        ProgramKind::Error => 4,
        ProgramKind::Working => 3,
        ProgramKind::Done => 2,
        ProgramKind::Idle | ProgramKind::Clear => 1,
    }
}

/// `Done` and `Failed` carry `Unseen` here; `presence_step` sets it from focus.
fn state_of(kind: ProgramKind) -> AgentState {
    match kind {
        ProgramKind::Working => AgentState::Working,
        ProgramKind::Done => AgentState::Done(Seen::Unseen),
        ProgramKind::Error => AgentState::Failed(Seen::Unseen),
        ProgramKind::Blocked(wait) => AgentState::Waiting(wait.unwrap_or(WaitKind::Question)),
        ProgramKind::Idle | ProgramKind::Clear => AgentState::Idle,
    }
}
