//! One order for a split's panes: the sidebar's segments and the focus steps (⌘]) both walk them
//! as they stand on the screen.

use temor_core::split::{Direction, Goto};
use temor_core::vocab::ids::TabId;
use temor_core::workspace::{WorkspaceIn, workspace_step};

use super::tests::hub_after;

#[test]
fn the_sidebar_lists_a_splits_panes_in_the_order_next_pane_steps_through_them() {
    let split = |towards| WorkspaceIn::Split { towards };
    // Down from the first pane, then the first pane again split to its right: the tree nests
    // them one way and the screen reads them another.
    let mut hub = hub_after(vec![
        WorkspaceIn::NewTab { cwd: None },
        split(Direction::Down),
        WorkspaceIn::Goto(Goto::Spatial(Direction::Up)),
        split(Direction::Right),
    ]);
    let tab = TabId(1);
    let listed: Vec<_> = hub.sidebar().loose[0]
        .tiles
        .iter()
        .map(|t| t.pane)
        .collect();
    assert_eq!(listed.len(), 3);
    assert_eq!(
        listed,
        hub.book
            .ws
            .tabs
            .tab(tab)
            .expect("the tab")
            .tree
            .reading_order()
    );

    let step = |hub: &mut super::model::Hub, input| {
        hub.book.ws = workspace_step(hub.book.ws.clone(), input).0;
    };
    step(&mut hub, WorkspaceIn::Focus(listed[0]));
    let mut walked = vec![hub.book.ws.focus_of(tab).expect("focus")];
    for _ in 1..listed.len() {
        step(&mut hub, WorkspaceIn::Goto(Goto::Next));
        walked.push(hub.book.ws.focus_of(tab).expect("focus"));
    }
    assert_eq!(walked, listed);
}
