//! Refusals of `workspace_step`: every refused request says why and changes nothing.

use super::step_tests::{one_tab, run};
use super::{
    RefuseWhy, TabRefusal, TabsIn, TabsOut, Workspace, WorkspaceIn, WorkspaceOut, workspace_step,
};
use crate::split::{Columns, Direction};
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::{Relaunch, TabTitle};

/// Tab 1 holds `first`; tab 2 holds panes 2 and 3 and is pinned.
fn two_tabs() -> (Workspace, PaneId) {
    let (ws, first) = one_tab();
    let (ws, _) = run(
        ws,
        vec![
            WorkspaceIn::NewTab { cwd: None },
            WorkspaceIn::Split {
                towards: Direction::Right,
            },
            WorkspaceIn::Tabs(TabsIn::Pin {
                tab: TabId(2),
                relaunch: Relaunch("x".into()),
            }),
        ],
    );
    (ws, first)
}

fn refused(why: RefuseWhy) -> WorkspaceOut {
    WorkspaceOut::Refused(why)
}

#[test]
fn refused_requests_say_why_and_change_nothing() {
    let (ws, first) = two_tabs();
    let (just_one, only) = one_tab();
    #[rustfmt::skip]
    let cases: Vec<(&str, &Workspace, WorkspaceIn, WorkspaceOut)> = vec![
        ("drop onto its own pane", &ws, WorkspaceIn::DropTab { from: TabId(1), onto: first, towards: Direction::Right }, refused(RefuseWhy::SameTab)),
        ("a pinned tab keeps its pin when dropped", &ws, WorkspaceIn::DropTab { from: TabId(2), onto: first, towards: Direction::Right }, refused(RefuseWhy::PinnedTab(TabId(2)))),
        ("drop a tab that is not there", &ws, WorkspaceIn::DropTab { from: TabId(9), onto: first, towards: Direction::Right }, refused(RefuseWhy::NoSuchTab(TabId(9)))),
        ("drop onto a pane that is not there", &ws, WorkspaceIn::DropTab { from: TabId(1), onto: PaneId(77), towards: Direction::Right }, refused(RefuseWhy::NoSuchPane(PaneId(77)))),
        ("tear a tab's only pane", &just_one, WorkspaceIn::Tear(only), refused(RefuseWhy::OnlyPane(only))),
        ("tear from a pinned tab", &ws, WorkspaceIn::Tear(PaneId(3)), refused(RefuseWhy::PinnedTab(TabId(2)))),
        ("tear a pane that is not there", &ws, WorkspaceIn::Tear(PaneId(77)), refused(RefuseWhy::NoSuchPane(PaneId(77)))),
        ("focus an unknown pane", &ws, WorkspaceIn::Focus(PaneId(77)), refused(RefuseWhy::NoSuchPane(PaneId(77)))),
        ("remove through the tab list", &ws, WorkspaceIn::Tabs(TabsIn::Remove(TabId(1))), refused(RefuseWhy::UseCloseTab)),
        ("close a tab that is not there", &ws, WorkspaceIn::CloseTab(TabId(9)), refused(RefuseWhy::NoSuchTab(TabId(9)))),
        ("rename a tab that is not there", &ws, WorkspaceIn::Tabs(TabsIn::Rename { tab: TabId(9), title: TabTitle("x".into()) }), WorkspaceOut::Tab(TabsOut::Refused(TabRefusal::NoSuchTab(TabId(9))))),
    ];
    for (name, start, input, want) in cases {
        let (after, out) = run(start.clone(), vec![input]);
        assert_eq!(out, vec![want], "{name}");
        assert_eq!(&after, start, "{name}: a refused request changes nothing");
    }
}

#[test]
fn ids_at_the_top_of_the_range_refuse_instead_of_overflowing() {
    let (mut split_ws, _) = one_tab();
    split_ws = run(
        split_ws,
        vec![WorkspaceIn::Split {
            towards: Direction::Right,
        }],
    )
    .0;
    let torn = PaneId(2);
    let with = |edit: fn(&mut Workspace)| {
        let mut ws = split_ws.clone();
        edit(&mut ws);
        ws
    };
    #[rustfmt::skip]
    let cases: Vec<(&str, Workspace, WorkspaceIn, WorkspaceOut)> = vec![
        ("a new tab with no pane id left", with(|ws| ws.next_pane = u64::MAX), WorkspaceIn::NewTab { cwd: None }, refused(RefuseWhy::IdsExhausted)),
        ("a new tab with no tab id left", with(|ws| ws.next_tab = u64::MAX), WorkspaceIn::NewTab { cwd: None }, refused(RefuseWhy::IdsExhausted)),
        ("a split with no pane id left", with(|ws| ws.next_pane = u64::MAX), WorkspaceIn::Split { towards: Direction::Down }, refused(RefuseWhy::IdsExhausted)),
        ("a side-by-side split of a pane under 120 columns", with(|_| {}), WorkspaceIn::SplitIn { towards: Direction::Right, width: Columns(119) }, refused(RefuseWhy::TooNarrow)),
        ("a tear with no tab id left", with(|ws| ws.next_tab = u64::MAX), WorkspaceIn::Tear(torn), refused(RefuseWhy::IdsExhausted)),
        ("a folder with no id left", with(|ws| ws.tabs.next_folder = u64::MAX), WorkspaceIn::Tabs(TabsIn::NewFolder { title: TabTitle("f".into()) }), WorkspaceOut::Tab(TabsOut::Refused(TabRefusal::IdsExhausted))),
    ];
    for (name, start, input, want) in cases {
        let (after, out) = workspace_step(start.clone(), input);
        assert_eq!(out, vec![want], "{name}");
        assert_eq!(after, start, "{name}: a refused request changes nothing");
    }
}

#[test]
fn a_full_tab_refuses_another_split() {
    let (mut ws, _) = one_tab();
    let mut out = Vec::new();
    for _ in 0..200 {
        let (next, o) = workspace_step(
            ws,
            WorkspaceIn::Split {
                towards: Direction::Right,
            },
        );
        ws = next;
        out = o;
    }
    assert_eq!(out, vec![refused(RefuseWhy::TabFull)]);
}

#[test]
fn a_roomy_pane_splits_the_same_whether_or_not_its_width_is_told() {
    let (ws, _) = one_tab();
    let plain = workspace_step(
        ws.clone(),
        WorkspaceIn::Split {
            towards: Direction::Right,
        },
    );
    let told = workspace_step(
        ws,
        WorkspaceIn::SplitIn {
            towards: Direction::Right,
            width: Columns(120),
        },
    );
    assert_eq!(plain, told);
}
