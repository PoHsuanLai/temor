//! `quick_step`: showing, hiding and promoting the quick terminal.

use super::model::{QuickIn, QuickOut, QuickState, QuickTerm};

/// The quick terminal after `input`. A report about a pane that is not the quick one is stale
/// and changes nothing; so does "Move to Space" with nothing to move.
pub fn quick_step(state: QuickState, input: QuickIn) -> (QuickState, Vec<QuickOut>) {
    match (state, input) {
        (QuickState::Closed, QuickIn::Toggle { pane, cwd }) => (
            QuickState::Open(QuickTerm {
                pane,
                cwd: cwd.clone(),
            }),
            vec![QuickOut::Spawn { pane, cwd }],
        ),
        (QuickState::Open(term), QuickIn::Toggle { .. }) => {
            (QuickState::Hidden(term), vec![QuickOut::Hidden])
        }
        (QuickState::Hidden(term), QuickIn::Toggle { .. }) => {
            let pane = term.pane;
            (QuickState::Open(term), vec![QuickOut::Show(pane)])
        }
        (QuickState::Open(term) | QuickState::Hidden(term), QuickIn::ShellExited(pane))
            if term.pane == pane =>
        {
            (
                QuickState::Closed,
                vec![QuickOut::End(pane), QuickOut::Hidden],
            )
        }
        (QuickState::Open(term), QuickIn::Cwd { pane, cwd }) if term.pane == pane => (
            QuickState::Open(QuickTerm {
                cwd: Some(cwd),
                ..term
            }),
            vec![],
        ),
        (QuickState::Hidden(term), QuickIn::Cwd { pane, cwd }) if term.pane == pane => (
            QuickState::Hidden(QuickTerm {
                cwd: Some(cwd),
                ..term
            }),
            vec![],
        ),
        (QuickState::Open(term) | QuickState::Hidden(term), QuickIn::MoveToSpace(space)) => (
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
