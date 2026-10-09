//! Scenarios over `workspace_step`: each test runs a short sequence of requests and checks what
//! the window is told. Table rows cover the refusals.

use super::{
    ClosedState, KeepClosed, RefuseWhy, Spawn, Start, TabSel, TabsIn, TabsOut, Workspace,
    WorkspaceIn, WorkspaceOut, workspace_step,
};
use crate::space::Pin;
use crate::split::Direction;
use crate::vocab::ids::{PaneId, TabId};
use crate::vocab::text::{Cwd, Relaunch};

fn fresh() -> Workspace {
    Workspace::new(ClosedState::new(KeepClosed::DEFAULT))
}

fn run(ws: Workspace, inputs: Vec<WorkspaceIn>) -> (Workspace, Vec<WorkspaceOut>) {
    inputs
        .into_iter()
        .fold((ws, Vec::new()), |(ws, mut all), input| {
            let (next, out) = workspace_step(ws, input);
            all.extend(out);
            (next, all)
        })
}

fn one_tab() -> (Workspace, PaneId) {
    let (ws, out) = run(
        fresh(),
        vec![WorkspaceIn::NewTab {
            cwd: Some(Cwd("/home/a".into())),
        }],
    );
    let pane = match out[0] {
        WorkspaceOut::Spawn(Spawn { pane, .. }) => pane,
        ref other => panic!("expected a spawn, got {other:?}"),
    };
    (ws, pane)
}

fn spawns(out: &[WorkspaceOut]) -> Vec<Spawn> {
    out.iter()
        .filter_map(|o| match o {
            WorkspaceOut::Spawn(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn new_tab_spawns_a_shell_in_the_folder_and_is_in_front() {
    let (ws, out) = run(
        fresh(),
        vec![WorkspaceIn::NewTab {
            cwd: Some(Cwd("/srv".into())),
        }],
    );
    let s = spawns(&out);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].cwd, Some(Cwd("/srv".into())));
    assert_eq!(s[0].start, Start::Fresh);
    assert_eq!(ws.front(), Some(TabId(1)));
    assert!(out.contains(&WorkspaceOut::Focused(PaneId(1))));
    assert_eq!(out.last(), Some(&WorkspaceOut::Changed));
}

#[test]
fn split_right_adds_a_pane_that_starts_where_its_origin_is() {
    let (ws, first) = one_tab();
    let (ws, out) = run(
        ws,
        vec![WorkspaceIn::Split {
            towards: Direction::Right,
        }],
    );
    let s = spawns(&out);
    assert_eq!(s.len(), 1);
    assert_ne!(s[0].pane, first);
    assert_eq!(s[0].cwd, Some(Cwd("/home/a".into())));
    assert_eq!(ws.focus_of(TabId(1)), Some(s[0].pane));
    assert_eq!(ws.tabs.tab(TabId(1)).unwrap().tree.panes().len(), 2);
}

#[test]
fn closing_a_split_pane_keeps_the_tab_and_snapshots_the_pane_first() {
    let (ws, first) = one_tab();
    let (ws, _) = run(
        ws,
        vec![WorkspaceIn::Split {
            towards: Direction::Right,
        }],
    );
    let (ws, out) = run(ws, vec![WorkspaceIn::ClosePane(Some(first))]);
    assert_eq!(out[0], WorkspaceOut::Snapshot(first));
    assert_eq!(out[1], WorkspaceOut::End(first));
    assert_eq!(ws.tabs.place.tabs.len(), 1);
    assert_eq!(ws.tabs.tab(TabId(1)).unwrap().tree.panes().len(), 1);
    assert!(ws.closed.tabs.is_empty());
}

#[test]
fn closing_the_last_pane_closes_the_tab_into_recently_closed() {
    let (ws, first) = one_tab();
    let (ws, out) = run(ws, vec![WorkspaceIn::ClosePane(Some(first))]);
    assert!(out.contains(&WorkspaceOut::Snapshot(first)));
    assert!(out.contains(&WorkspaceOut::End(first)));
    assert!(ws.tabs.place.tabs.is_empty());
    assert_eq!(ws.front(), None);
    assert_eq!(ws.closed.tabs.len(), 1);
}

#[test]
fn reopen_brings_the_tab_back_on_its_scrollback_in_its_folder() {
    let (ws, first) = one_tab();
    let (ws, _) = run(ws, vec![WorkspaceIn::CloseTab(TabId(1))]);
    let (ws, out) = run(ws, vec![WorkspaceIn::Reopen]);
    let s = spawns(&out);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].pane, first);
    assert_eq!(s[0].start, Start::Scrollback(first));
    assert_eq!(s[0].cwd, Some(Cwd("/home/a".into())));
    assert_eq!(ws.front(), Some(TabId(1)));
    assert_eq!(ws.focus_of(TabId(1)), Some(first));
    assert!(ws.closed.tabs.is_empty());
}

#[test]
fn reopening_a_pinned_tab_runs_its_command_again() {
    let (ws, _) = one_tab();
    let (ws, _) = run(
        ws,
        vec![
            WorkspaceIn::Tabs(TabsIn::Pin {
                tab: TabId(1),
                relaunch: Relaunch("htop".into()),
            }),
            WorkspaceIn::CloseTab(TabId(1)),
            WorkspaceIn::Reopen,
        ],
    );
    assert_eq!(
        ws.tabs.tab(TabId(1)).unwrap().pin,
        Pin::Pinned {
            relaunch: Relaunch("htop".into())
        }
    );
    let (_, out) = run(
        ws.clone(),
        vec![WorkspaceIn::CloseTab(TabId(1)), WorkspaceIn::Reopen],
    );
    assert_eq!(spawns(&out)[0].relaunch, Some(Relaunch("htop".into())));
}

#[test]
fn a_full_reopen_with_nothing_closed_is_refused() {
    let (_, out) = run(fresh(), vec![WorkspaceIn::Reopen]);
    assert_eq!(out, vec![WorkspaceOut::Refused(RefuseWhy::NothingClosed)]);
}

#[test]
fn the_oldest_closed_tab_is_forgotten_past_the_limit() {
    let keep_one = Workspace::new(ClosedState::new(KeepClosed::new(1).unwrap()));
    let (ws, out) = run(keep_one, vec![WorkspaceIn::NewTab { cwd: None }]);
    let first = spawns(&out)[0].pane;
    let (ws, _) = run(ws, vec![WorkspaceIn::CloseTab(TabId(1))]);
    let (ws, second) = run(ws, vec![WorkspaceIn::NewTab { cwd: None }]);
    let second_pane = spawns(&second)[0].pane;
    let (ws, out) = run(ws, vec![WorkspaceIn::CloseTab(TabId(2))]);
    assert!(out.contains(&WorkspaceOut::Forgotten(vec![first])));
    assert_eq!(ws.closed.tabs.len(), 1);
    assert_eq!(ws.closed.tabs[0].focus, second_pane);
}

#[test]
fn dropping_a_tab_onto_a_pane_merges_it_and_keeps_its_focus() {
    let (ws, first) = one_tab();
    let (ws, _) = run(ws, vec![WorkspaceIn::NewTab { cwd: None }]);
    let second = PaneId(2);
    let (ws, out) = run(
        ws,
        vec![WorkspaceIn::DropTab {
            from: TabId(2),
            onto: first,
            towards: Direction::Right,
        }],
    );
    assert!(out.contains(&WorkspaceOut::Focused(second)));
    assert_eq!(ws.tabs.place.tabs.len(), 1);
    let merged = ws.tabs.tab(TabId(1)).unwrap();
    assert_eq!(merged.tree.panes(), vec![first, second]);
    assert_eq!(ws.focus_of(TabId(1)), Some(second));
    assert!(ws.closed.tabs.is_empty(), "a dropped tab is not closed");
}

#[test]
fn tab_drops_and_tears_refuse_the_bad_cases() {
    let (ws, first) = one_tab();
    let (ws, _) = run(ws, vec![WorkspaceIn::NewTab { cwd: None }]);
    let (ws, _) = run(
        ws,
        vec![WorkspaceIn::Tabs(TabsIn::Pin {
            tab: TabId(2),
            relaunch: Relaunch("x".into()),
        })],
    );
    #[rustfmt::skip]
    let cases: Vec<(&str, WorkspaceIn, RefuseWhy)> = vec![
        ("onto its own pane", WorkspaceIn::DropTab { from: TabId(1), onto: first, towards: Direction::Right }, RefuseWhy::SameTab),
        ("a pinned tab keeps its pin", WorkspaceIn::DropTab { from: TabId(2), onto: first, towards: Direction::Right }, RefuseWhy::PinnedTab(TabId(2))),
        ("tear the only pane", WorkspaceIn::Tear(first), RefuseWhy::OnlyPane(first)),
        ("focus an unknown pane", WorkspaceIn::Focus(PaneId(77)), RefuseWhy::NoSuchPane(PaneId(77))),
        ("remove through the tab list", WorkspaceIn::Tabs(TabsIn::Remove(TabId(1))), RefuseWhy::UseCloseTab),
    ];
    for (name, input, why) in cases {
        let (after, out) = run(ws.clone(), vec![input]);
        assert!(out.contains(&WorkspaceOut::Refused(why)), "{name}: {out:?}");
        assert_eq!(after, ws, "{name}: a refused request changes nothing");
    }
}

#[test]
fn tearing_a_pane_makes_its_own_tab_and_focuses_it() {
    let (ws, first) = one_tab();
    let (ws, _) = run(
        ws,
        vec![WorkspaceIn::Split {
            towards: Direction::Right,
        }],
    );
    let second = ws.focus_of(TabId(1)).unwrap();
    let (ws, out) = run(ws, vec![WorkspaceIn::Tear(second)]);
    assert_eq!(out.last(), Some(&WorkspaceOut::Changed));
    assert_eq!(ws.tabs.place.tabs.len(), 2);
    assert_eq!(ws.front(), Some(TabId(2)));
    assert_eq!(ws.focus_of(TabId(2)), Some(second));
    assert_eq!(ws.tabs.tab(TabId(1)).unwrap().tree.panes(), vec![first]);
}

#[test]
fn a_command_ending_in_a_background_tab_marks_it_unread() {
    let (ws, first) = one_tab();
    let (ws, _) = run(ws, vec![WorkspaceIn::NewTab { cwd: None }]);
    let (ws, out) = run(ws, vec![WorkspaceIn::Activity(first)]);
    assert!(ws.tabs.unread.contains(&TabId(1)));
    assert_eq!(out, vec![WorkspaceOut::Changed]);
    let (ws, out) = run(
        ws,
        vec![WorkspaceIn::Tabs(TabsIn::Select(TabSel::Id(TabId(1))))],
    );
    assert!(!ws.tabs.unread.contains(&TabId(1)));
    assert!(
        out.contains(&WorkspaceOut::Focused(first))
            || out
                .iter()
                .any(|o| matches!(o, WorkspaceOut::Tab(TabsOut::Focused(TabId(1)))))
    );
    let (_, out) = run(ws, vec![WorkspaceIn::Activity(first)]);
    assert!(out.is_empty(), "an ignored change is not a change");
}

#[test]
fn zoom_toggles_and_a_split_needs_a_tab_in_front() {
    let (ws, _) = run(
        fresh(),
        vec![WorkspaceIn::Split {
            towards: Direction::Down,
        }],
    );
    assert_eq!(ws.front(), None);
    let (ws, _) = one_tab();
    let (ws, _) = run(
        ws,
        vec![WorkspaceIn::Split {
            towards: Direction::Right,
        }],
    );
    let (ws, _) = run(ws, vec![WorkspaceIn::ToggleZoom]);
    assert!(ws.tabs.tab(TabId(1)).unwrap().tree.zoomed().is_some());
    let (ws, _) = run(ws, vec![WorkspaceIn::ToggleZoom]);
    assert!(ws.tabs.tab(TabId(1)).unwrap().tree.zoomed().is_none());
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
    assert_eq!(out, vec![WorkspaceOut::Refused(RefuseWhy::TabFull)]);
}

#[test]
fn selecting_tabs_by_number_follows_the_sidebar() {
    let (ws, _) = one_tab();
    let (ws, _) = run(
        ws,
        vec![
            WorkspaceIn::NewTab { cwd: None },
            WorkspaceIn::NewTab { cwd: None },
        ],
    );
    let (ws, _) = run(
        ws,
        vec![WorkspaceIn::Tabs(TabsIn::Pin {
            tab: TabId(3),
            relaunch: Relaunch("x".into()),
        })],
    );
    let (ws, out) = run(ws, vec![WorkspaceIn::Tabs(TabsIn::Select(TabSel::Next))]);
    assert_eq!(ws.front(), Some(TabId(1)));
    assert!(
        out.iter()
            .any(|o| matches!(o, WorkspaceOut::Tab(TabsOut::Focused(_))))
    );
}
