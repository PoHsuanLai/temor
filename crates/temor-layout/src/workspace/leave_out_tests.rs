//! `leave_out`: the workspace as saved without some panes.

use super::step_tests::{one_tab, run};
use super::{Workspace, WorkspaceIn, leave_out};
use crate::split::Direction;
use temor_core::vocab::ids::TabId;

#[test]
fn a_pane_left_out_leaves_its_split_and_a_tab_left_empty_goes_without_a_trace() {
    let (ws, first) = one_tab();
    let (ws, _) = run(
        ws,
        vec![
            WorkspaceIn::Split {
                towards: Direction::Right,
            },
            WorkspaceIn::NewTab { cwd: None },
        ],
    );
    let panes = |ws: &Workspace, tab: u64| ws.tabs.tab(TabId(tab)).map(|t| t.tree.panes());
    let split = panes(&ws, 1).expect("the split tab");
    let alone = panes(&ws, 2).expect("the second tab")[0];

    let kept = leave_out(&ws, &[first, alone].into_iter().collect());
    let other = split
        .into_iter()
        .find(|p| *p != first)
        .expect("the other half");
    assert_eq!(kept.tabs.place.tabs.len(), 1);
    assert_eq!(panes(&kept, 1), Some(vec![other]));
    assert_eq!(kept.focus_of(TabId(1)), Some(other));
    assert!(
        kept.closed.tabs.is_empty(),
        "an emptied tab is not put in the recently closed"
    );
    assert!(!kept.cwds.contains_key(&first));
    // The live workspace is untouched.
    assert_eq!(ws.tabs.place.tabs.len(), 2);
}
