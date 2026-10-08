//! `bar_step`: the ⌘T bar from opening to a picked row (§7).
//!
//! While open, the bar's own field has focus, so the keys it is given were never the shell's;
//! the bar takes no Ctrl-C at all. Selection stops at the ends and does not wrap. Rows come
//! ranked from [`rank`]; the output tells quire's palette machine when they changed.

use super::model::{BarRow, Candidate, PathResolver, PluginRow, RowKey};
use super::rank::rank;
use crate::vocab::ai::AiSwitch;

/// The bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarState {
    /// Not shown.
    Closed,
    /// Shown, with a query and its rows.
    Open(OpenBar),
    /// A row was picked; the bar is done.
    Picked(BarRow),
}

/// An open bar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenBar {
    /// What the person typed.
    pub query: String,
    /// The plugin rows for `query`.
    pub plugins: Vec<PluginRow>,
    /// The ranked rows.
    pub rows: Vec<BarRow>,
    /// The highlighted row.
    pub highlighted: Option<RowKey>,
}

/// A key in the bar's field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BarKey {
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Tab: the next row.
    Tab,
    /// Shift-Tab: the row before.
    BackTab,
    /// Enter: pick the highlighted row.
    Enter,
    /// Escape: close the bar.
    Escape,
}

/// What moves the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarIn {
    /// ⌘T: opens a closed bar, closes an open one.
    Toggle,
    /// The query changed.
    Typed(String),
    /// Plugins answered for `query`.
    ProviderRows {
        /// The query they answered.
        query: String,
        /// Their rows.
        rows: Vec<PluginRow>,
    },
    /// A key in the field.
    Key(BarKey),
    /// A row was clicked.
    Click(RowKey),
}

/// What the bar wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarOut {
    /// The rows changed: send the palette `Refreshed`.
    Refreshed,
    /// Ask the plugins for rows for this query.
    AskProviders(String),
    /// Carry out the picked row.
    Run(BarRow),
    /// The bar closed.
    Closed,
}

/// What a step reads but does not own: the local candidates, the `PATH` lookup and the AI switch.
#[derive(Clone, Copy)]
pub struct BarContext<'a> {
    /// Spaces, tabs, folders, hosts, layouts, recent commands and folders.
    pub candidates: &'a [Candidate],
    /// The `PATH` lookup.
    pub resolver: &'a dyn PathResolver,
    /// Whether AI rows may show.
    pub ai: AiSwitch,
}

impl std::fmt::Debug for BarContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BarContext")
            .field("candidates", &self.candidates)
            .field("ai", &self.ai)
            .finish_non_exhaustive()
    }
}

/// The bar after `input`.
pub fn bar_step(state: BarState, input: BarIn, cx: BarContext<'_>) -> (BarState, Vec<BarOut>) {
    match (state, input) {
        (BarState::Open(_), BarIn::Toggle) | (BarState::Open(_), BarIn::Key(BarKey::Escape)) => {
            (BarState::Closed, vec![BarOut::Closed])
        }
        (BarState::Closed | BarState::Picked(_), BarIn::Toggle) => {
            let bar = reranked(OpenBar::default(), cx);
            (BarState::Open(bar), vec![BarOut::Refreshed])
        }
        (BarState::Open(bar), BarIn::Typed(query)) => {
            let ask = BarOut::AskProviders(query.clone());
            let bar = reranked(
                OpenBar {
                    query,
                    plugins: Vec::new(),
                    ..bar
                },
                cx,
            );
            (BarState::Open(bar), vec![ask, BarOut::Refreshed])
        }
        (BarState::Open(bar), BarIn::ProviderRows { query, rows }) if query == bar.query => {
            let bar = reranked(
                OpenBar {
                    plugins: rows,
                    ..bar
                },
                cx,
            );
            (BarState::Open(bar), vec![BarOut::Refreshed])
        }
        (BarState::Open(bar), BarIn::Key(BarKey::Up | BarKey::BackTab)) => moved(bar, Move::Back),
        (BarState::Open(bar), BarIn::Key(BarKey::Down | BarKey::Tab)) => moved(bar, Move::On),
        (BarState::Open(bar), BarIn::Key(BarKey::Enter)) => match bar.highlighted.clone() {
            Some(key) => pick(bar, &key),
            None => (BarState::Open(bar), vec![]),
        },
        (BarState::Open(bar), BarIn::Click(key)) => pick(bar, &key),
        (state, _) => (state, vec![]),
    }
}

/// `bar` with its rows ranked afresh, keeping the highlight if that row is still there.
fn reranked(bar: OpenBar, cx: BarContext<'_>) -> OpenBar {
    let rows = rank(&bar.query, cx.candidates, &bar.plugins, cx.resolver, cx.ai);
    let highlighted = bar
        .highlighted
        .filter(|key| rows.iter().any(|row| row.key == *key))
        .or_else(|| rows.first().map(|row| row.key.clone()));
    OpenBar {
        rows,
        highlighted,
        ..bar
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    Back,
    On,
}

fn moved(bar: OpenBar, way: Move) -> (BarState, Vec<BarOut>) {
    let at = bar
        .highlighted
        .as_ref()
        .and_then(|key| bar.rows.iter().position(|row| row.key == *key));
    let to = match (at, way) {
        (None, _) => 0,
        (Some(at), Move::Back) => at.saturating_sub(1),
        (Some(at), Move::On) => (at + 1).min(bar.rows.len().saturating_sub(1)),
    };
    let highlighted = bar.rows.get(to).map(|row| row.key.clone());
    (BarState::Open(OpenBar { highlighted, ..bar }), vec![])
}

fn pick(bar: OpenBar, key: &RowKey) -> (BarState, Vec<BarOut>) {
    match bar.rows.iter().find(|row| row.key == *key) {
        Some(row) => (
            BarState::Picked(row.clone()),
            vec![BarOut::Run(row.clone())],
        ),
        None => (BarState::Open(bar), vec![]),
    }
}
