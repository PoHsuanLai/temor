//! Identifiers and counts: each a newtype over an integer, so an argument-order mistake is a
//! compile error and a stored id reads as a plain number.

use serde::{Deserialize, Serialize};

/// Defines a transparent integer newtype with the derives every identifier carries.
macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident($inner:ty)) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);
    };
}

id_newtype!(
    /// One pane: a terminal, an agent view or a viewer, unique within a window's session.
    PaneId(u64)
);
id_newtype!(
    /// One tab in a Space's sidebar.
    TabId(u64)
);
id_newtype!(
    /// One command block (an OSC 133 prompt, command and output).
    BlockId(u64)
);
id_newtype!(
    /// A folder that groups tabs in the sidebar.
    FolderId(u64)
);
id_newtype!(
    /// One item on the downloads shelf.
    DownloadId(u64)
);
id_newtype!(
    /// A durable session the host's helper keeps for one pane.
    RemoteSessionId(u64)
);
id_newtype!(
    /// The model turn that produced a proposal, as the companion session record names it.
    TurnRef(u64)
);
id_newtype!(
    /// An agent-view session (an ACP session through docket), as docket numbers it.
    AgentSessionRef(u64)
);
id_newtype!(
    /// A status report's place in its pane's sequence: a larger one is newer.
    Seq(u64)
);
id_newtype!(
    /// How many times a reconnect has been tried since the link dropped.
    Attempt(u32)
);
id_newtype!(
    /// A merge's undo number: `refs/temor/undo/<n>`.
    UndoRef(u64)
);
id_newtype!(
    /// A count of bytes.
    Bytes(u64)
);
id_newtype!(
    /// A process exit status as the shell reports it (OSC 133 D).
    ExitCode(i32)
);
id_newtype!(
    /// A row of the terminal grid, counted from the top of the scrollback.
    GridRow(u64)
);

impl ExitCode {
    /// The status a successful command exits with.
    pub const SUCCESS: ExitCode = ExitCode(0);

    /// Whether the command succeeded.
    pub fn succeeded(self) -> bool {
        self == Self::SUCCESS
    }

    /// Whether the command was stopped by the person rather than failing: 130 (Ctrl-C) or
    /// 141 (a closed pipe).
    pub fn stopped(self) -> bool {
        matches!(self.0, 130 | 141)
    }
}

impl Seq {
    /// Whether a report carrying `self` is newer than one carrying `seen`.
    pub fn is_newer_than(self, seen: Seq) -> bool {
        self > seen
    }
}

/// A span of grid rows, first to last inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GridSpan {
    /// The first row.
    pub first: GridRow,
    /// The last row, never before `first`.
    pub last: GridRow,
}

impl GridSpan {
    /// The span over one row.
    pub fn row(row: GridRow) -> GridSpan {
        GridSpan {
            first: row,
            last: row,
        }
    }

    /// This span, grown (never shrunk) to reach `row`.
    pub fn reaching(self, row: GridRow) -> GridSpan {
        GridSpan {
            first: self.first.min(row),
            last: self.last.max(row),
        }
    }
}

/// Hands out ids in order, each once: it holds the id it gives next. It never gives `u64::MAX`,
/// so the counter after any id it gave can be held, and a refusal leaves it as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Counter(pub u64);

impl Default for Counter {
    fn default() -> Self {
        Counter::FROM_ONE
    }
}

impl Counter {
    /// A counter that starts at 1, leaving 0 for "none".
    pub const FROM_ONE: Counter = Counter(1);

    /// The id it gives next and the counter after it; none when no id is left.
    pub fn mint(self) -> Option<(u64, Counter)> {
        let after = self.0.checked_add(1)?;
        Some((self.0, Counter(after)))
    }

    /// Takes the id it gives next; none, and the counter unchanged, when no id is left.
    pub fn take(&mut self) -> Option<u64> {
        let (id, after) = self.mint()?;
        *self = after;
        Some(id)
    }

    /// The counter that gives nothing at or below `id`; none when no id is left after it.
    pub fn past(self, id: u64) -> Option<Counter> {
        Some(Counter(self.0.max(id.checked_add(1)?)))
    }

    /// The counter that gives nothing any of `ids` holds.
    pub fn past_all(self, ids: impl IntoIterator<Item = u64>) -> Option<Counter> {
        ids.into_iter().try_fold(self, Counter::past)
    }
}

#[cfg(test)]
mod tests {
    use super::Counter;

    #[test]
    fn a_counter_gives_each_id_once_in_order() {
        let mut c = Counter::FROM_ONE;
        assert_eq!((c.take(), c.take()), (Some(1), Some(2)));
        assert_eq!(c, Counter(3));
    }

    #[test]
    fn a_counter_with_no_id_left_refuses_and_stays() {
        let mut c = Counter(u64::MAX);
        assert_eq!(c.take(), None);
        assert_eq!(c, Counter(u64::MAX));
        assert_eq!(
            Counter(u64::MAX - 1).mint(),
            Some((u64::MAX - 1, Counter(u64::MAX)))
        );
    }

    #[test]
    fn past_moves_only_forward_and_refuses_the_last_id() {
        assert_eq!(Counter(5).past(2), Some(Counter(5)));
        assert_eq!(Counter(5).past(9), Some(Counter(10)));
        assert_eq!(Counter(5).past_all([3, 7, 4]), Some(Counter(8)));
        assert_eq!(Counter(5).past(u64::MAX), None);
    }
}
