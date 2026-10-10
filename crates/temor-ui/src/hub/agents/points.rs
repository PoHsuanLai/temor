//! A run's restore points as the window keeps them and the tab menu shows them (PLAN P19).
//! docket keeps the points; the window holds the last list it was given for each pane and the
//! port to ask for a new one. Pure: the calls are `checkpoints.rs`.

use std::collections::BTreeMap;
use std::sync::Arc;

use temor_core::presence::AgentState;
use temor_core::vocab::ids::PaneId;
use temor_desktop::{
    CheckpointPort, NoCheckpoints, NotSavedWhy, PointId, PointList, PointRow, PointState,
    SessionKey, TurnNo,
};

/// The most rows the menu lists; the newest are kept.
const LISTED: usize = 12;

/// Said where Restore is greyed out because the agent is still working.
pub const WAIT_FOR_AGENT: &str = "Wait for the agent to finish";

/// What the window knows of the restore points of each agent pane.
pub struct Points {
    /// Where the lists come from; nothing until the desktop is joined.
    pub(super) port: Arc<dyn CheckpointPort>,
    /// The docket session each agent pane runs, once known.
    pub(super) sessions: BTreeMap<PaneId, SessionKey>,
    /// The last list read for each pane.
    pub(super) lists: BTreeMap<PaneId, PointList>,
}

impl Default for Points {
    fn default() -> Self {
        Points {
            port: Arc::new(NoCheckpoints),
            sessions: BTreeMap::new(),
            lists: BTreeMap::new(),
        }
    }
}

impl Points {
    /// The pane is gone: its session and list are forgotten.
    pub(super) fn forget(&mut self, pane: PaneId) {
        self.sessions.remove(&pane);
        self.lists.remove(&pane);
    }

    /// The turn that `id` precedes, from the pane's last list.
    pub(super) fn turn_of(&self, pane: PaneId, id: PointId) -> Option<TurnNo> {
        self.lists.get(&pane).and_then(|list| turn_of(list, id))
    }
}

fn turn_of(list: &PointList, id: PointId) -> Option<TurnNo> {
    list.rows.iter().find_map(|row| match row {
        PointRow::Saved { id: at, turn, .. } if *at == id => Some(*turn),
        _ => None,
    })
}

/// One line of the Restore submenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointLine {
    /// A point that can be restored.
    Choice { id: PointId, label: String },
    /// A line that says something and picks nothing: a turn that was not saved, a point that
    /// was cleared, a restore that happened.
    Note(String),
}

/// What the tab menu offers for Restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreMenu {
    /// Nothing to show: no session is known for the pane, or its list is empty.
    Absent,
    /// The agent is still working: the entry is shown but takes no pick.
    Waiting,
    /// The run's points and the notes between them.
    Lines(Vec<PointLine>),
}

/// The Restore entry for a pane in `state` whose last list is `list`.
pub fn restore_menu(list: Option<&PointList>, state: AgentState) -> RestoreMenu {
    let Some(list) = list.filter(|list| !list.rows.is_empty()) else {
        return RestoreMenu::Absent;
    };
    if state == AgentState::Working {
        return RestoreMenu::Waiting;
    }
    let skip = list.rows.len().saturating_sub(LISTED);
    RestoreMenu::Lines(
        list.rows
            .iter()
            .skip(skip)
            .map(|row| line_of(list, row))
            .collect(),
    )
}

fn before(turn: TurnNo) -> String {
    format!("Before turn {}", turn.0)
}

fn reason(why: NotSavedWhy) -> &'static str {
    match why {
        NotSavedWhy::AgentKeepsOwn => "this agent keeps its own history",
        NotSavedWhy::NoHistory => "this folder is not in a git repository",
        NotSavedWhy::TooLarge => "this folder has too many files",
        NotSavedWhy::Failed => "it could not be saved",
        NotSavedWhy::Unknown => "no reason was given",
    }
}

fn line_of(list: &PointList, row: &PointRow) -> PointLine {
    match *row {
        PointRow::Saved {
            id,
            turn,
            state: PointState::Available,
            ..
        } => PointLine::Choice {
            id,
            label: before(turn),
        },
        PointRow::Saved { turn, .. } => PointLine::Note(format!("{}: cleared", before(turn))),
        PointRow::NotSaved { turn, why, .. } => {
            PointLine::Note(format!("Turn {} not saved: {}", turn.0, reason(why)))
        }
        PointRow::RestoredTo { id, .. } => PointLine::Note(match turn_of(list, id) {
            Some(turn) => format!("Put back to {}", before(turn).to_lowercase()),
            None => "Put back to an earlier point".to_owned(),
        }),
    }
}
