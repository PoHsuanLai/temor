//! Counting clicks (PLAN §6: a double-click selects a word, a triple-click a line). Pure: the
//! caller passes the time since the start of the program and where the press landed.

use std::time::Duration;

use temor_pane::text::CellPos;

/// How close in time two presses must be to count as one multi-click.
pub const MULTI_CLICK: Duration = Duration::from_millis(500);

/// How many presses in a row, at one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Click {
    Single,
    Double,
    Triple,
}

/// The last press: when it was and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickState {
    at: Duration,
    cell: CellPos,
    count: Click,
}

/// The click a press at `cell` at time `now` makes, and the state to remember for the next one.
/// A press more than [`MULTI_CLICK`] after the last, or at another cell, starts again; a fourth
/// press starts again as a single.
pub fn next_click(last: Option<ClickState>, now: Duration, cell: CellPos) -> (Click, ClickState) {
    let count = match last {
        Some(prev) if prev.cell == cell && now.saturating_sub(prev.at) <= MULTI_CLICK => {
            match prev.count {
                Click::Single => Click::Double,
                Click::Double => Click::Triple,
                Click::Triple => Click::Single,
            }
        }
        _ => Click::Single,
    };
    (
        count,
        ClickState {
            at: now,
            cell,
            count,
        },
    )
}
