//! The bar's providers that need no window: what the recently closed list, the blocks and the
//! folders contribute, as plain data for [`temor_client::model::bar::candidates`].

use std::collections::BTreeMap;

use temor_client::model::bar::{Candidate, DirectoryUse, Target, UnixSecs, Visits};
use temor_client::model::vocab::text::{CommandText, Cwd};
use temor_client::model::workspace::ClosedState;

/// The most recent commands the bar remembers.
const RECENT_COMMANDS: usize = 50;

/// The tab closed last, as a row that brings it back with its scrollback. Only the newest: a
/// reopen takes the newest closed tab (`WorkspaceIn::Reopen`).
pub fn closed_candidate(closed: &ClosedState) -> Option<Candidate> {
    let last = closed.tabs.last()?;
    let title = last.tab.title.as_str();
    (!title.trim().is_empty()).then(|| Candidate {
        title: format!("Reopen {title}"),
        target: Target::Closed(last.seq),
    })
}

/// `commands` (newest first, from every pane's blocks) without repeats, safe to type, at most
/// [`RECENT_COMMANDS`].
pub fn recent_commands(commands: impl IntoIterator<Item = CommandText>) -> Vec<CommandText> {
    let mut out: Vec<CommandText> = Vec::new();
    for command in commands {
        if command.typable() && !out.contains(&command) {
            out.push(command);
        }
        if out.len() == RECENT_COMMANDS {
            break;
        }
    }
    out
}

/// The folders to offer: the ones opened from the bar (`visits`), plus each folder a pane is in
/// now that has no count yet, as one visit at `now`.
pub fn directory_uses<'a>(
    visits: &BTreeMap<String, Visits>,
    panes_in: impl IntoIterator<Item = &'a Cwd>,
    now: UnixSecs,
) -> Vec<DirectoryUse> {
    let mut all = visits.clone();
    for cwd in panes_in {
        all.entry(cwd.as_str().to_owned()).or_insert(Visits {
            count: 1,
            last: now,
        });
    }
    all.into_iter()
        .map(|(cwd, visits)| DirectoryUse {
            cwd: Cwd(cwd),
            visits,
        })
        .collect()
}
