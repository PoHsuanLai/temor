//! `quick_step`: opening, closing and promoting the quick terminal.

use super::model::{QuickIn, QuickOut, QuickState, QuickTerm};

/// The quick terminal after `input`. A report about a pane that is not the open one is stale and
/// changes nothing; so does "Move to Space" with nothing open.
pub fn quick_step(state: QuickState, input: QuickIn) -> (QuickState, Vec<QuickOut>) {
    match (state, input) {
        (QuickState::Closed, QuickIn::Toggle { pane, cwd }) => (
            QuickState::Open(QuickTerm {
                pane,
                cwd: cwd.clone(),
            }),
            vec![QuickOut::Spawn { pane, cwd }],
        ),
        (QuickState::Open(term), QuickIn::Toggle { .. }) => (
            QuickState::Closed,
            vec![QuickOut::End(term.pane), QuickOut::Hidden],
        ),
        (QuickState::Open(term), QuickIn::ShellExited(pane)) if term.pane == pane => {
            (QuickState::Closed, vec![QuickOut::Hidden])
        }
        (QuickState::Open(term), QuickIn::Cwd { pane, cwd }) if term.pane == pane => (
            QuickState::Open(QuickTerm {
                cwd: Some(cwd),
                ..term
            }),
            vec![],
        ),
        (QuickState::Open(term), QuickIn::MoveToSpace(space)) => (
            QuickState::Closed,
            vec![
                QuickOut::Promote {
                    pane: term.pane,
                    space,
                    cwd: term.cwd,
                },
                QuickOut::Hidden,
            ],
        ),
        (state, _) => (state, vec![]),
    }
}
