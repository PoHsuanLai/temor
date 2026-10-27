//! Generated request sequences over `workspace_step`. The generator is seeded the same way on
//! every run, so a failure is a failure of the code, and it replays. After every step:
//! every tab's tree is a valid tree, focus sits inside its tree, tabs and panes are unique,
//! the id counters stand past every id ever seen, and a refused request left the state as it
//! was.

use std::collections::BTreeSet;

use ds_core::word::Word;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

use super::{
    ClosedState, KeepClosed, NthTab, TabSel, TabsIn, TabsOut, Workspace, WorkspaceIn, WorkspaceOut,
    workspace_step,
};
use crate::split::{Direction, Goto, SplitTree};
use crate::vocab::ids::{FolderId, PaneId, TabId};
use crate::vocab::text::{Cwd, Relaunch, TabTitle};

/// A request as the generator writes it: indexes, resolved against the workspace it meets.
/// An index that lands past the end names something that does not exist.
#[derive(Debug, Clone)]
enum Op {
    NewTab,
    Split(Direction),
    ClosePane(usize),
    CloseFront,
    CloseTab(usize),
    Goto(Goto),
    ToggleZoom,
    Equalize,
    Focus(usize),
    Drop(usize, usize, Direction),
    Tear(usize),
    Activity(usize),
    Cwd(usize),
    Reopen,
    Select(usize, u8),
    Pin(usize),
    Unpin(usize),
    NewFolder,
    ToFolder(usize, usize),
    Rename(usize),
    Remove(usize),
}

const GOTOS: [Goto; 6] = [
    Goto::Previous,
    Goto::Next,
    Goto::PreviousWrapped,
    Goto::NextWrapped,
    Goto::Spatial(Direction::Left),
    Goto::Spatial(Direction::Down),
];

fn op() -> impl Strategy<Value = Op> {
    let dir = || prop::sample::select(Direction::ALL.to_vec());
    let at = || 0usize..12;
    prop_oneof![
        4 => Just(Op::NewTab),
        6 => dir().prop_map(Op::Split),
        3 => at().prop_map(Op::ClosePane),
        1 => Just(Op::CloseFront),
        2 => at().prop_map(Op::CloseTab),
        2 => prop::sample::select(GOTOS.to_vec()).prop_map(Op::Goto),
        1 => Just(Op::ToggleZoom),
        1 => Just(Op::Equalize),
        2 => at().prop_map(Op::Focus),
        3 => (at(), at(), dir()).prop_map(|(a, b, d)| Op::Drop(a, b, d)),
        3 => at().prop_map(Op::Tear),
        1 => at().prop_map(Op::Activity),
        1 => at().prop_map(Op::Cwd),
        2 => Just(Op::Reopen),
        2 => (at(), 0u8..12).prop_map(|(a, n)| Op::Select(a, n)),
        1 => at().prop_map(Op::Pin),
        1 => at().prop_map(Op::Unpin),
        1 => Just(Op::NewFolder),
        1 => (at(), at()).prop_map(|(a, b)| Op::ToFolder(a, b)),
        1 => at().prop_map(Op::Rename),
        1 => at().prop_map(Op::Remove),
    ]
}

fn pane_at(ws: &Workspace, at: usize) -> PaneId {
    let panes = ws.tabs.panes();
    panes.get(at).copied().unwrap_or(PaneId(9_000 + at as u64))
}

fn tab_at(ws: &Workspace, at: usize) -> TabId {
    let tabs = &ws.tabs.place.tabs;
    tabs.get(at).map_or(TabId(9_000 + at as u64), |t| t.id)
}

fn request(ws: &Workspace, op: &Op) -> WorkspaceIn {
    match *op {
        Op::NewTab => WorkspaceIn::NewTab { cwd: None },
        Op::Split(towards) => WorkspaceIn::Split { towards },
        Op::ClosePane(at) => WorkspaceIn::ClosePane(Some(pane_at(ws, at))),
        Op::CloseFront => WorkspaceIn::ClosePane(None),
        Op::CloseTab(at) => WorkspaceIn::CloseTab(tab_at(ws, at)),
        Op::Goto(to) => WorkspaceIn::Goto(to),
        Op::ToggleZoom => WorkspaceIn::ToggleZoom,
        Op::Equalize => WorkspaceIn::Equalize,
        Op::Focus(at) => WorkspaceIn::Focus(pane_at(ws, at)),
        Op::Drop(from, onto, towards) => WorkspaceIn::DropTab {
            from: tab_at(ws, from),
            onto: pane_at(ws, onto),
            towards,
        },
        Op::Tear(at) => WorkspaceIn::Tear(pane_at(ws, at)),
        Op::Activity(at) => WorkspaceIn::Activity(pane_at(ws, at)),
        Op::Cwd(at) => WorkspaceIn::Cwd {
            pane: pane_at(ws, at),
            cwd: Cwd(format!("/p/{at}")),
        },
        Op::Reopen => WorkspaceIn::Reopen,
        Op::Select(at, kind) => WorkspaceIn::Tabs(TabsIn::Select(match kind {
            0..=5 => TabSel::Id(tab_at(ws, at)),
            6 | 7 => TabSel::Next,
            8 | 9 => TabSel::Previous,
            n => NthTab::new(n - 9).map_or(TabSel::Next, TabSel::Nth),
        })),
        Op::Pin(at) => WorkspaceIn::Tabs(TabsIn::Pin {
            tab: tab_at(ws, at),
            relaunch: Relaunch("top".into()),
        }),
        Op::Unpin(at) => WorkspaceIn::Tabs(TabsIn::Unpin(tab_at(ws, at))),
        Op::NewFolder => WorkspaceIn::Tabs(TabsIn::NewFolder {
            title: TabTitle("f".into()),
            parent: None,
        }),
        Op::ToFolder(at, folder) => WorkspaceIn::Tabs(TabsIn::MoveToFolder {
            tab: tab_at(ws, at),
            folder: (folder < 8).then_some(FolderId(folder as u64 + 1)),
        }),
        Op::Rename(at) => WorkspaceIn::Tabs(TabsIn::Rename {
            tab: tab_at(ws, at),
            title: TabTitle("renamed".into()),
        }),
        Op::Remove(at) => WorkspaceIn::Tabs(TabsIn::Remove(tab_at(ws, at))),
    }
}

/// The largest ids any state has held so far.
#[derive(Default)]
struct Seen {
    pane: u64,
    tab: u64,
}

impl Seen {
    fn note(&mut self, ws: &Workspace) {
        let closed = ws.closed.tabs.iter().map(|c| &c.tab);
        let all = ws.tabs.place.tabs.iter().chain(closed);
        for tab in all {
            self.tab = self.tab.max(tab.id.0);
            for pane in tab.tree.panes() {
                self.pane = self.pane.max(pane.0);
            }
        }
    }
}

fn check(ws: &Workspace, seen: &mut Seen) -> Result<(), TestCaseError> {
    seen.note(ws);
    let tabs = &ws.tabs.place.tabs;
    let tab_ids: BTreeSet<TabId> = tabs.iter().map(|t| t.id).collect();
    prop_assert_eq!(tab_ids.len(), tabs.len(), "tab ids are unique");
    let live = ws.tabs.panes();
    let live_set: BTreeSet<PaneId> = live.iter().copied().collect();
    prop_assert_eq!(live_set.len(), live.len(), "panes are unique across tabs");
    for tab in tabs {
        let again = SplitTree::from_nodes(tab.tree.nodes().to_vec(), tab.tree.zoomed());
        prop_assert!(again.is_ok(), "tab {:?} holds a valid tree", tab.id);
        let focus = ws.focus_of(tab.id);
        prop_assert!(
            focus.is_some_and(|p| tab.tree.contains(p)),
            "tab {:?} focuses a pane of its own tree",
            tab.id
        );
    }
    for (tab, pane) in &ws.focus {
        let inside = ws.tabs.tab(*tab).is_some_and(|t| t.tree.contains(*pane));
        prop_assert!(inside, "a recorded focus names a live tab and its pane");
    }
    match ws.front() {
        Some(front) => prop_assert!(tab_ids.contains(&front), "the front tab is in the list"),
        None => prop_assert!(tabs.is_empty(), "with tabs open, one is in front"),
    }
    prop_assert!(
        ws.tabs.unread.is_subset(&tab_ids),
        "unread marks name live tabs"
    );
    let mut closed_panes = BTreeSet::new();
    for closed in &ws.closed.tabs {
        closed_panes.extend(closed.tab.tree.panes());
    }
    prop_assert!(
        closed_panes.is_disjoint(&live_set),
        "a pane is live or closed"
    );
    let known: BTreeSet<PaneId> = live_set.union(&closed_panes).copied().collect();
    prop_assert!(
        ws.cwds.keys().all(|p| known.contains(p)),
        "folders are kept only for live or recently closed panes"
    );
    prop_assert!(
        ws.next_pane > seen.pane,
        "pane ids are never handed out twice"
    );
    prop_assert!(ws.next_tab > seen.tab, "tab ids are never handed out twice");
    Ok(())
}

#[test]
fn generated_sequences_keep_the_workspace_consistent() {
    let config = Config {
        cases: 160,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::deterministic_rng(RngAlgorithm::ChaCha));
    let ops = prop::collection::vec(op(), 1..70);
    let result = runner.run(&ops, |ops| {
        let keep = KeepClosed::new(3).expect("three is a valid limit");
        let mut ws = Workspace::new(ClosedState::new(keep));
        let mut seen = Seen::default();
        for op in &ops {
            let input = request(&ws, op);
            let (after, out) = workspace_step(ws.clone(), input);
            let refused = out.iter().any(|o| {
                matches!(
                    o,
                    WorkspaceOut::Refused(_) | WorkspaceOut::Tab(TabsOut::Refused(_))
                )
            });
            if refused {
                prop_assert_eq!(&after, &ws, "{:?} was refused but changed the state", op);
                prop_assert!(!out.contains(&WorkspaceOut::Changed));
            }
            ws = after;
            check(&ws, &mut seen)?;
        }
        Ok(())
    });
    result.unwrap();
}
