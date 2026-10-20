//! The quick terminal's scenarios and the adoption of its pane.

use super::{QuickIn, QuickOut, QuickState, QuickTerm, adopt, quick_step};
use crate::error::CoreError;
use crate::test_support::fresh_workspace;
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::Cwd;
use crate::workspace::{Workspace, WorkspaceIn, WorkspaceOut, workspace_step};
use ds_style::space::list::SpaceId;

fn cwd(path: &str) -> Option<Cwd> {
    Some(Cwd(path.to_owned()))
}

fn open(pane: u64, path: Option<&str>) -> QuickState {
    QuickState::Open(QuickTerm {
        pane: PaneId(pane),
        cwd: path.and_then(cwd),
    })
}

fn hidden(pane: u64, path: Option<&str>) -> QuickState {
    QuickState::Hidden(QuickTerm {
        pane: PaneId(pane),
        cwd: path.and_then(cwd),
    })
}

fn toggle(pane: u64, path: &str) -> QuickIn {
    QuickIn::Toggle {
        pane: PaneId(pane),
        cwd: cwd(path),
    }
}

#[test]
fn quick_terminal_steps() {
    #[rustfmt::skip]
    let cases: Vec<(&str, QuickState, QuickIn, QuickState, Vec<QuickOut>)> = vec![
        ("the hotkey opens a shell where the window is", QuickState::Closed, toggle(5, "/a"), open(5, Some("/a")),
            vec![QuickOut::Spawn { pane: PaneId(5), cwd: cwd("/a") }]),
        ("the hotkey again hides it and keeps the shell", open(5, Some("/a")), toggle(9, "/b"), hidden(5, Some("/a")),
            vec![QuickOut::Hidden]),
        ("the hotkey once more shows the same shell, not a new pane", hidden(5, Some("/a")), toggle(9, "/b"), open(5, Some("/a")),
            vec![QuickOut::Show(PaneId(5))]),
        ("the shell exiting drops its pane", open(5, None), QuickIn::ShellExited(PaneId(5)), QuickState::Closed,
            vec![QuickOut::End(PaneId(5)), QuickOut::Hidden]),
        ("the shell exiting while hidden drops it too", hidden(5, None), QuickIn::ShellExited(PaneId(5)), QuickState::Closed,
            vec![QuickOut::End(PaneId(5)), QuickOut::Hidden]),
        ("a folder report reaches a hidden shell", hidden(5, Some("/a")), QuickIn::Cwd { pane: PaneId(5), cwd: Cwd("/b".into()) }, hidden(5, Some("/b")), vec![]),
        ("Move to Space works from hidden", hidden(5, Some("/b")), QuickIn::MoveToSpace(SpaceId(2)), QuickState::Closed,
            vec![QuickOut::Promote { pane: PaneId(5), space: SpaceId(2), cwd: cwd("/b") }, QuickOut::Hidden]),
        ("another pane exiting is stale", open(5, None), QuickIn::ShellExited(PaneId(6)), open(5, None), vec![]),
        ("a folder report follows the shell", open(5, Some("/a")), QuickIn::Cwd { pane: PaneId(5), cwd: Cwd("/b".into()) }, open(5, Some("/b")), vec![]),
        ("a stale folder report is ignored", open(5, Some("/a")), QuickIn::Cwd { pane: PaneId(6), cwd: Cwd("/b".into()) }, open(5, Some("/a")), vec![]),
        ("Move to Space keeps the shell and hides the overlay", open(5, Some("/b")), QuickIn::MoveToSpace(SpaceId(2)), QuickState::Closed,
            vec![QuickOut::Promote { pane: PaneId(5), space: SpaceId(2), cwd: cwd("/b") }, QuickOut::Hidden]),
        ("Move to Space with nothing open", QuickState::Closed, QuickIn::MoveToSpace(SpaceId(2)), QuickState::Closed, vec![]),
        ("a report while closed", QuickState::Closed, QuickIn::ShellExited(PaneId(5)), QuickState::Closed, vec![]),
    ];
    for (name, state, input, want_state, want_out) in cases {
        assert_eq!(quick_step(state, input), (want_state, want_out), "{name}");
    }
}

fn workspace_with_two_panes() -> Workspace {
    let ws = fresh_workspace();
    let (ws, _) = workspace_step(ws, WorkspaceIn::NewTab { cwd: None });
    workspace_step(ws, WorkspaceIn::NewTab { cwd: None }).0
}

#[test]
fn a_promoted_pane_becomes_the_front_tab_and_the_counters_move_past_it() {
    let ws = workspace_with_two_panes();
    let (after, out) = adopt(&ws, PaneId(40), cwd("/work")).expect("a new pane");
    assert_eq!(
        out,
        vec![WorkspaceOut::Focused(PaneId(40)), WorkspaceOut::Changed]
    );
    assert_eq!(after.front(), Some(TabId(3)));
    assert_eq!(after.focus_of(TabId(3)), Some(PaneId(40)));
    assert_eq!(after.cwds.get(&PaneId(40)), cwd("/work").as_ref());
    assert_eq!((after.next_tab, after.next_pane), (4, 41));
    let (after, _) = workspace_step(after, WorkspaceIn::NewTab { cwd: None });
    assert_eq!(after.front(), Some(TabId(4)));
    assert_eq!(
        after.focus_of(TabId(4)),
        Some(PaneId(41)),
        "new panes do not reuse 40"
    );
}

#[test]
fn a_pane_already_in_a_tab_or_an_exhausted_counter_is_refused_and_nothing_changes() {
    let ws = workspace_with_two_panes();
    assert_eq!(
        adopt(&ws, PaneId(2), None),
        Err(CoreError::DuplicatePane(2))
    );
    let mut full = ws.clone();
    full.next_tab = u64::MAX;
    assert_eq!(adopt(&full, PaneId(40), None), Err(CoreError::IdsExhausted));
    assert_eq!(
        adopt(&ws, PaneId(u64::MAX), None),
        Err(CoreError::IdsExhausted)
    );
    assert_eq!(ws.tabs.place.tabs.len(), 2);
}
