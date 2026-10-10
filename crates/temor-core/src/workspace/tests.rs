use super::{
    NthTab, SidebarPlan, TabRefusal, TabSel, TabsIn, TabsOut, TabsState, order, sidebar_plan,
    tabs_step,
};
use crate::space::{Folder, Pin, Place, Tab};
use crate::split::SplitTree;
use crate::vocab::ids::{FolderId, PaneId, TabId};
use crate::vocab::text::{Relaunch, TabTitle};

fn tab(id: u64, pin: Pin, folder: Option<u64>) -> Tab {
    Tab {
        id: TabId(id),
        title: TabTitle(format!("tab {id}")),
        pin,
        folder: folder.map(FolderId),
        tree: SplitTree::single(PaneId(id)),
    }
}

/// t1 loose, t2 pinned, t3 in folder f1, t4 loose; t3 in front.
fn fixture() -> TabsState {
    TabsState::from_place(Place {
        tabs: vec![
            tab(1, Pin::Loose, None),
            tab(
                2,
                Pin::Pinned {
                    relaunch: Relaunch("htop".into()),
                },
                None,
            ),
            tab(3, Pin::Loose, Some(1)),
            tab(4, Pin::Loose, None),
        ],
        folders: vec![Folder {
            id: FolderId(1),
            title: TabTitle("Work".into()),
        }],
        focused: Some(TabId(3)),
        viewers: vec![],
    })
}

fn ids(v: &[u64]) -> Vec<TabId> {
    v.iter().map(|n| TabId(*n)).collect()
}

#[test]
fn sidebar_order_is_pinned_then_folders_then_the_rest() {
    let state = fixture();
    assert_eq!(order(&state), ids(&[2, 3, 1, 4]));
    assert_eq!(
        sidebar_plan(&state),
        SidebarPlan {
            pinned: ids(&[2]),
            folders: vec![(
                Folder {
                    id: FolderId(1),
                    title: TabTitle("Work".into())
                },
                ids(&[3]),
            )],
            loose: ids(&[1, 4]),
        }
    );
}

#[test]
fn a_tab_in_a_missing_folder_is_still_reachable() {
    let mut state = fixture();
    state.place.folders.clear();
    assert_eq!(order(&state), ids(&[2, 1, 4, 3]));
}

#[test]
fn tab_list_steps() {
    // (name, tab in front before, request, tab in front after, the last output)
    #[rustfmt::skip]
    let cases: Vec<(&str, u64, TabsIn, u64, TabsOut)> = vec![
        ("select by id", 3, TabsIn::Select(TabSel::Id(TabId(4))), 4, TabsOut::Focused(TabId(4))),
        ("select unknown id", 3, TabsIn::Select(TabSel::Id(TabId(9))), 3, TabsOut::Refused(TabRefusal::NoSuchTab(TabId(9)))),
        ("nth 1 is the first pinned", 3, TabsIn::Select(TabSel::Nth(NthTab::new(1).unwrap())), 2, TabsOut::Focused(TabId(2))),
        ("nth 3 follows the sidebar", 3, TabsIn::Select(TabSel::Nth(NthTab::new(3).unwrap())), 1, TabsOut::Focused(TabId(1))),
        ("nth 5 has no tab", 3, TabsIn::Select(TabSel::Nth(NthTab::new(5).unwrap())), 3, TabsOut::Refused(TabRefusal::NoTabAtSlot(NthTab::new(5).unwrap()))),
        ("next moves down the sidebar", 2, TabsIn::Select(TabSel::Next), 3, TabsOut::Focused(TabId(3))),
        ("next wraps from the last", 4, TabsIn::Select(TabSel::Next), 2, TabsOut::Focused(TabId(2))),
        ("previous wraps from the first", 2, TabsIn::Select(TabSel::Previous), 4, TabsOut::Focused(TabId(4))),
    ];
    for (name, start, input, focus_after, out) in cases {
        let mut state = fixture();
        state.place.focused = Some(TabId(start));
        let (after, outs) = tabs_step(state, input);
        assert_eq!(
            after.place.focused,
            Some(TabId(focus_after)),
            "{name}: focus"
        );
        assert_eq!(outs.last(), Some(&out), "{name}: output");
    }
}

#[test]
fn pin_unpin_folder_and_rename_edit_the_tab() {
    let (state, outs) = tabs_step(
        fixture(),
        TabsIn::Pin {
            tab: TabId(1),
            relaunch: Relaunch("make".into()),
        },
    );
    assert!(outs.is_empty());
    assert_eq!(order(&state)[0], TabId(1));
    let (state, _) = tabs_step(state, TabsIn::Unpin(TabId(2)));
    assert_eq!(sidebar_plan(&state).pinned, ids(&[1]));
    let (state, outs) = tabs_step(
        state,
        TabsIn::MoveToFolder {
            tab: TabId(4),
            folder: Some(FolderId(1)),
        },
    );
    assert!(outs.is_empty());
    assert!(sidebar_plan(&state).folders[0].1.contains(&TabId(4)));
    let (_, outs) = tabs_step(
        state.clone(),
        TabsIn::MoveToFolder {
            tab: TabId(4),
            folder: Some(FolderId(7)),
        },
    );
    assert_eq!(
        outs,
        vec![TabsOut::Refused(TabRefusal::NoSuchFolder(FolderId(7)))]
    );
    let (state, _) = tabs_step(
        state,
        TabsIn::Rename {
            tab: TabId(4),
            title: TabTitle("new".into()),
        },
    );
    assert_eq!(state.tab(TabId(4)).unwrap().title, TabTitle("new".into()));
}

#[test]
fn new_folders_take_the_next_id() {
    let (state, outs) = tabs_step(
        fixture(),
        TabsIn::NewFolder {
            title: TabTitle("Ops".into()),
        },
    );
    assert_eq!(outs, vec![TabsOut::Created(FolderId(2))]);
    assert_eq!(state.place.folders.len(), 2);
}

#[test]
fn unread_is_kept_for_background_tabs_and_cleared_by_selecting() {
    let (state, outs) = tabs_step(fixture(), TabsIn::Unread(TabId(1)));
    assert!(outs.is_empty());
    assert!(state.unread.contains(&TabId(1)));
    let (state, _) = tabs_step(state, TabsIn::Select(TabSel::Id(TabId(1))));
    assert!(!state.unread.contains(&TabId(1)));
    let (state, _) = tabs_step(state, TabsIn::Unread(TabId(1)));
    assert!(
        state.unread.is_empty(),
        "the tab in front gets no unread mark"
    );
}

#[test]
fn removing_the_front_tab_focuses_its_neighbour() {
    let (state, outs) = tabs_step(fixture(), TabsIn::Remove(TabId(3)));
    assert_eq!(outs[0], TabsOut::Removed(tab(3, Pin::Loose, Some(1))));
    assert_eq!(outs[1], TabsOut::Focused(TabId(1)));
    assert_eq!(state.place.focused, Some(TabId(1)));
}

#[test]
fn removing_the_last_tab_leaves_nothing_in_front() {
    let mut state = fixture();
    state.place.tabs = vec![tab(9, Pin::Loose, None)];
    state.place.focused = Some(TabId(9));
    let (state, outs) = tabs_step(state, TabsIn::Remove(TabId(9)));
    assert_eq!(state.place.focused, None);
    assert_eq!(outs.len(), 1);
    let (_, outs) = tabs_step(state, TabsIn::Select(TabSel::Next));
    assert_eq!(outs, vec![TabsOut::Refused(TabRefusal::NoTabs)]);
}
