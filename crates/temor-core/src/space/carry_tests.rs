//! Moving a tab between workspaces.

use super::carry_tab;
use crate::error::CoreError;
use crate::space::Pin;
use crate::split::Direction;
use crate::test_support::{fresh_workspace, run_steps};
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::Cwd;
use crate::workspace::{Workspace, WorkspaceIn, workspace_step};

fn built(steps: Vec<WorkspaceIn>) -> Workspace {
    run_steps(fresh_workspace(), steps, workspace_step).0
}

fn tab_in(cwd: &str) -> WorkspaceIn {
    WorkspaceIn::NewTab {
        cwd: Some(Cwd(cwd.to_owned())),
    }
}

#[test]
fn a_tab_keeps_its_panes_and_folders_and_takes_a_new_id_in_the_other_workspace() {
    // Work has tab 1 (pane 1), then tab 2 split in two (panes 2 and 3); Home has tab 1 (pane 1).
    let work = built(vec![
        tab_in("/work/a"),
        tab_in("/work/b"),
        WorkspaceIn::Split {
            towards: Direction::Right,
        },
    ]);
    let mut home = built(vec![tab_in("/home/x")]);
    // Pane ids are the window's to keep apart; Home's counters are already past Work's panes.
    home.next_pane = 10;
    let (work_after, home_after) = carry_tab(&work, TabId(2), &home).expect("moved");

    assert_eq!(work_after.tabs.place.tabs.len(), 1);
    assert_eq!(work_after.tabs.place.focused, Some(TabId(1)));
    assert!(work_after.cwds.contains_key(&PaneId(1)));
    assert!(!work_after.cwds.contains_key(&PaneId(2)));

    let moved = home_after
        .tabs
        .tab(TabId(2))
        .expect("a new tab id from Home");
    assert_eq!(moved.tree.panes(), [PaneId(2), PaneId(3)]);
    assert_eq!(moved.pin, Pin::Loose);
    assert_eq!(home_after.focus_of(TabId(2)), work.focus_of(TabId(2)));
    assert_eq!(home_after.cwds.get(&PaneId(2)), work.cwds.get(&PaneId(2)));
    assert_eq!(
        home_after.tabs.place.focused,
        Some(TabId(1)),
        "Home's front stays"
    );
    assert_eq!(
        (home_after.next_tab, home_after.next_pane),
        (3, 10),
        "counters stay ahead of what is in use"
    );
}

#[test]
fn an_empty_workspace_shows_the_tab_it_is_given_and_a_missing_tab_is_refused() {
    let work = built(vec![tab_in("/w")]);
    let empty = fresh_workspace();
    let (work_after, got) = carry_tab(&work, TabId(1), &empty).expect("moved");
    assert!(work_after.tabs.place.tabs.is_empty());
    assert_eq!(got.tabs.place.focused, Some(TabId(1)));
    assert_eq!(got.next_pane, 2);

    assert_eq!(
        carry_tab(&work, TabId(9), &empty),
        Err(CoreError::UnknownTab(9))
    );
}
