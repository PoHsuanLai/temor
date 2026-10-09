//! Recently closed tabs: the last few tabs closed, newest last, each kept with its focus so a
//! reopen puts it back as it was. Only the most recent `keep` are kept; an older one is evicted
//! and its panes are reported so their stored scrollback can go.

use std::num::NonZeroU8;

use crate::space::Tab;
use crate::vocab::ids::{PaneId, Seq};

/// How many closed tabs are kept. At least one; the default is ten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeepClosed(NonZeroU8);

impl KeepClosed {
    /// The default count.
    pub const DEFAULT: KeepClosed = match NonZeroU8::new(10) {
        Some(ten) => KeepClosed(ten),
        None => KeepClosed(NonZeroU8::MIN),
    };

    /// A count of `n`, when `n` is at least one.
    pub fn new(n: u8) -> Option<KeepClosed> {
        NonZeroU8::new(n).map(KeepClosed)
    }

    /// The count.
    pub fn get(self) -> usize {
        usize::from(self.0.get())
    }
}

impl Default for KeepClosed {
    fn default() -> Self {
        KeepClosed::DEFAULT
    }
}

/// A tab that was closed, with the pane that had focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedTab {
    /// The tab as it was.
    pub tab: Tab,
    /// The pane that had focus in it.
    pub focus: PaneId,
    /// The order it was closed in.
    pub seq: Seq,
}

/// The recently closed list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedState {
    /// Oldest first.
    pub tabs: Vec<ClosedTab>,
    /// How many to keep.
    pub keep: KeepClosed,
    /// The sequence number the next closed tab takes.
    pub next_seq: u64,
}

impl ClosedState {
    /// An empty list keeping `keep` tabs.
    pub fn new(keep: KeepClosed) -> ClosedState {
        ClosedState {
            tabs: Vec::new(),
            keep,
            next_seq: 0,
        }
    }
}

/// What the recently closed list is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedIn {
    /// A tab was closed.
    Push {
        /// The tab as it was.
        tab: Tab,
        /// The pane that had focus.
        focus: PaneId,
    },
    /// Take the newest closed tab back.
    Reopen,
}

/// What the recently closed list tells the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedOut {
    /// Older tabs were dropped; their panes' stored scrollback can go.
    Evicted(Vec<PaneId>),
    /// The tab taken back.
    Reopened(ClosedTab),
    /// There is no closed tab to take back.
    NothingToReopen,
}

/// The recently closed list after `input`.
pub fn closed_step(mut state: ClosedState, input: ClosedIn) -> (ClosedState, Vec<ClosedOut>) {
    let out = match input {
        ClosedIn::Push { tab, focus } => {
            let seq = Seq(state.next_seq);
            state.next_seq = state.next_seq.saturating_add(1);
            state.tabs.push(ClosedTab { tab, focus, seq });
            let mut evicted = Vec::new();
            while state.tabs.len() > state.keep.get() {
                let gone = state.tabs.remove(0);
                evicted.extend(gone.tab.tree.panes());
            }
            if evicted.is_empty() {
                Vec::new()
            } else {
                vec![ClosedOut::Evicted(evicted)]
            }
        }
        ClosedIn::Reopen => match state.tabs.pop() {
            Some(closed) => vec![ClosedOut::Reopened(closed)],
            None => vec![ClosedOut::NothingToReopen],
        },
    };
    (state, out)
}
